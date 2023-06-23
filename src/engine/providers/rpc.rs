use std::ops::RangeBounds;
use std::sync::{Arc, Mutex};

use ethers::providers::{Middleware, Provider};
use ethers::types::{
    Block as ethersBlock, BlockId as ethersBlockId,
    Transaction as ethersTransaction,
    TransactionReceipt as ethersTransactionReceipt, TxHash as ethersTxHash,
    H256 as ethersH256,
};
use ethers_providers::JsonRpcClient;
use ethers_providers::{Http, Ws};
use futures::future::join_all;
use reqwest::header::HeaderMap;
use reqwest::{Client, Url};
use reth_interfaces::Error as rethError;
use reth_interfaces::Result as rethResult;
use reth_network_api::NetworkError;
use reth_primitives::{
    Account, Address, BlockHash, BlockHashOrNumber, BlockNumber, Bytecode,
    Bytes, ChainInfo, ChainSpec, ChainSpecBuilder, Header, Receipt,
    SealedHeader, StorageKey, StorageValue, TransactionMeta, TransactionSigned,
    TransactionSignedNoHash, TxHash, TxNumber,
};
use reth_provider::{
    AccountReader, BlockHashProvider, BlockIdProvider, BlockNumProvider,
    EvmEnvProvider, HeaderProvider, PostState, ProviderError, ReceiptProvider,
    StateProvider, StateProviderFactory, StateRootProvider,
    TransactionsProvider,
};
use revm_primitives::{BlockEnv, CfgEnv, HashMap, B256 as H256, U256};

use crate::config::flags::SoflConfig;
use crate::utils::conversion::{Convert, ToEthers, ToIterator, ToPrimitive};

use super::BcProviderBuilder;

#[derive(Debug, Clone)]
pub enum JsonRpcError {
    InvalidEndpoint(String),
}

impl BcProviderBuilder {
    pub fn with_jsonrpc_via_http(
        url: String,
    ) -> Result<JsonRpcBcProvider<Http>, JsonRpcError> {
        BcProviderBuilder::with_jsonrpc_via_http_with_auth(url, None)
    }
    pub fn with_jsonrpc_via_http_with_auth(
        url: String,
        auth: impl Into<Option<HeaderMap>>,
    ) -> Result<JsonRpcBcProvider<Http>, JsonRpcError> {
        // TODO: use retry client
        let auth: Option<HeaderMap> = auth.into();
        let provider;
        if let Some(auth) = auth {
            let mut headers = HeaderMap::new();
            headers.extend::<HeaderMap>(auth);
            let client = Client::builder()
                .default_headers(headers)
                .build()
                .map_err(|_| JsonRpcError::InvalidEndpoint(url.clone()))?;
            let url = Url::parse(url.as_str())
                .map_err(|_| JsonRpcError::InvalidEndpoint(url))?;
            let http_provider = Http::new_with_client(url, client);
            provider = Provider::<Http>::new(http_provider);
        } else {
            provider = Provider::<Http>::try_from(&url)
                .map_err(|_| JsonRpcError::InvalidEndpoint(url))?;
        }
        let provider = Arc::new(provider);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        Ok(JsonRpcBcProvider { provider, runtime })
    }

    pub fn with_jsonrpc_via_ws(
        url: String,
    ) -> Result<JsonRpcBcProvider<Ws>, JsonRpcError> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let ws_provider = runtime
            .block_on(Ws::connect(url.as_str()))
            .map_err(|_| JsonRpcError::InvalidEndpoint(url.clone()))?;
        let provider = Arc::new(Provider::<Ws>::new(ws_provider));
        Ok(JsonRpcBcProvider { provider, runtime })
    }
}

pub struct JsonRpcBcProvider<P: JsonRpcClient> {
    pub(crate) provider: Arc<Provider<P>>,
    runtime: tokio::runtime::Runtime,
}

impl Default for JsonRpcBcProvider<Http> {
    fn default() -> Self {
        let cfg = SoflConfig::load().expect("failed to load config");
        let url = cfg.jsonrpc.endpoint.clone();
        BcProviderBuilder::with_jsonrpc_via_http_with_auth(url, cfg.jsonrpc)
            .expect("failed to create json-rpc provider from config")
    }
}

impl<P: JsonRpcClient> BlockHashProvider for JsonRpcBcProvider<P> {
    #[doc = " Get the hash of the block with the given number. Returns `None` if no block with this number"]
    #[doc = " exists."]
    fn block_hash(&self, number: BlockNumber) -> rethResult<Option<H256>> {
        let h = self
            .runtime
            .block_on(self.provider.get_block(number))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?
            .and_then(|b| b.hash)
            .map(|h| h.0.into());
        Ok(h)
    }

    #[doc = " Get headers in range of block hashes or numbers"]
    fn canonical_hashes_range(
        &self,
        start: BlockNumber,
        end: BlockNumber,
    ) -> rethResult<Vec<H256>> {
        let get_block = |n| async move {
            let b: rethResult<Option<ethersBlock<ethersTxHash>>> =
                self.provider.get_block(n).await.map_err(|_| {
                    rethError::Network(NetworkError::ChannelClosed)
                });
            b
        };
        let fs = (start..end).map(get_block);
        let bs = self.runtime.block_on(join_all(fs));
        let mut hs = Vec::new();
        // if any one of the blocks is not found, return an error
        for b in bs.iter() {
            if let Ok(ob) = b {
                if let Some(b) = ob {
                    if let Some(h) = b.hash {
                        hs.push(h.0.into());
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                return Err(rethError::Network(NetworkError::ChannelClosed));
            }
        }
        Ok(hs)
    }
}

impl<P: JsonRpcClient> BlockNumProvider for JsonRpcBcProvider<P> {
    #[doc = " Returns the current info for the chain."]
    fn chain_info(&self) -> rethResult<ChainInfo> {
        let bn = self.last_block_number()?;
        let h = self.block_hash(bn)?.unwrap();
        Ok(ChainInfo {
            best_hash: h,
            best_number: 0,
        })
    }

    #[doc = " Returns the best block number in the chain."]
    fn best_block_number(&self) -> rethResult<BlockNumber> {
        self.last_block_number()
    }

    #[doc = " Returns the last block number associated with the last canonical header in the database."]
    fn last_block_number(&self) -> rethResult<BlockNumber> {
        let bn = self
            .runtime
            .block_on(self.provider.get_block_number())
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?
            .as_u64();
        Ok(bn)
    }

    #[doc = " Gets the `BlockNumber` for the given hash. Returns `None` if no block with this hash exists."]
    fn block_number(&self, hash: H256) -> rethResult<Option<BlockNumber>> {
        let block_id: ethersBlockId = ToEthers::cvt(&hash);
        let h = self
            .runtime
            .block_on(self.provider.get_block(block_id))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?
            .and_then(|b| b.number)
            .map(|n| n.as_u64());
        Ok(h)
    }
}

impl<P: JsonRpcClient> TransactionsProvider for JsonRpcBcProvider<P> {
    #[doc = " Get internal transaction identifier by transaction hash."]
    #[doc = ""]
    #[doc = " This is the inverse of [TransactionsProvider::transaction_by_id]."]
    #[doc = " Returns None if the transaction is not found."]
    fn transaction_id(&self, _tx_hash: TxHash) -> rethResult<Option<TxNumber>> {
        todo!()
    }

    #[doc = " Get transaction by id."]
    fn transaction_by_id(
        &self,
        _id: TxNumber,
    ) -> rethResult<Option<TransactionSigned>> {
        todo!()
    }

    #[doc = " Get transaction by transaction hash."]
    fn transaction_by_hash(
        &self,
        hash: TxHash,
    ) -> rethResult<Option<TransactionSigned>> {
        let tx = self
            .runtime
            .block_on(
                self.provider
                    .get_transaction::<ethersH256>(ToEthers::cvt(&hash)),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if tx.is_none() {
            return Ok(None);
        }
        let tx = tx.unwrap();
        let tx = ToPrimitive::cvt(&tx);
        Ok(Some(tx))
    }

    #[doc = " Get transaction by transaction hash and additional metadata of the block the transaction was"]
    #[doc = " mined in"]
    fn transaction_by_hash_with_meta(
        &self,
        hash: TxHash,
    ) -> rethResult<Option<(TransactionSigned, TransactionMeta)>> {
        // TODO: parallelize rpc calls
        let tx = self.transaction_by_hash(hash)?;
        if tx.is_none() {
            return Ok(None);
        }
        let tx = tx.unwrap();
        let receipt: Option<ethersTransactionReceipt> = self
            .runtime
            .block_on(
                self.provider.get_transaction_receipt::<ethersTxHash>(
                    ToEthers::cvt(&hash),
                ),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if receipt.is_none() {
            return Ok(None);
        }
        let receipt = receipt.unwrap();
        let block: Option<ethersBlock<ethersTxHash>> = self
            .runtime
            .block_on(self.provider.get_block(receipt.block_hash.unwrap()))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        let meta = TransactionMeta {
            tx_hash: tx.hash,
            index: receipt.transaction_index.as_u64(),
            block_hash: receipt.block_hash.unwrap().0.into(),
            block_number: receipt.block_number.unwrap().as_u64(),
            base_fee: block.base_fee_per_gas.map(|f| f.as_u64()),
        };
        Ok(Some((tx, meta)))
    }

    #[doc = " Get transaction block number"]
    fn transaction_block(
        &self,
        _id: TxNumber,
    ) -> rethResult<Option<BlockNumber>> {
        todo!()
    }

    #[doc = " Get transactions by block id."]
    fn transactions_by_block(
        &self,
        block: BlockHashOrNumber,
    ) -> rethResult<Option<Vec<TransactionSigned>>> {
        let block_id: ethersBlockId = ToEthers::cvt(&block);
        let block: Option<ethersBlock<ethersTransaction>> = self
            .runtime
            .block_on(self.provider.get_block_with_txs(block_id))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        let txs = block.transactions.iter().map(ToPrimitive::cvt).collect();
        Ok(Some(txs))
    }

    #[doc = " Get transactions by block range."]
    fn transactions_by_block_range(
        &self,
        range: impl RangeBounds<BlockNumber>,
    ) -> rethResult<Vec<Vec<TransactionSigned>>> {
        let mut bs = Vec::new();
        for bn in ToIterator::cvt(range) {
            let b =
                self.transactions_by_block(BlockHashOrNumber::Number(bn))?;
            if b.is_none() {
                break;
            }
            let b = b.unwrap();
            bs.push(b);
        }
        Ok(bs)
    }

    #[doc = " Get transactions by tx range."]
    fn transactions_by_tx_range(
        &self,
        _range: impl RangeBounds<TxNumber>,
    ) -> rethResult<Vec<TransactionSignedNoHash>> {
        todo!()
    }

    #[doc = " Get Senders from a tx range."]
    fn senders_by_tx_range(
        &self,
        _range: impl RangeBounds<TxNumber>,
    ) -> rethResult<Vec<Address>> {
        todo!()
    }

    #[doc = " Get transaction sender."]
    #[doc = ""]
    #[doc = " Returns None if the transaction is not found."]
    fn transaction_sender(&self, _id: TxNumber) -> rethResult<Option<Address>> {
        todo!()
    }
}

impl<P: JsonRpcClient> ReceiptProvider for JsonRpcBcProvider<P> {
    #[doc = " Get receipt by transaction number"]
    fn receipt(&self, _id: TxNumber) -> rethResult<Option<Receipt>> {
        todo!()
    }

    #[doc = " Get receipt by transaction hash."]
    fn receipt_by_hash(&self, hash: TxHash) -> rethResult<Option<Receipt>> {
        let receipt: Option<ethersTransactionReceipt> = self
            .runtime
            .block_on(
                self.provider.get_transaction_receipt::<ethersTxHash>(
                    ToEthers::cvt(&hash),
                ),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if receipt.is_none() {
            return Ok(None);
        }
        let receipt = receipt.unwrap();
        Ok(Some(ToPrimitive::cvt(&receipt)))
    }

    #[doc = " Get receipts by block num or hash."]
    fn receipts_by_block(
        &self,
        block: BlockHashOrNumber,
    ) -> rethResult<Option<Vec<Receipt>>> {
        let block = self
            .runtime
            .block_on(
                self.provider
                    .get_block::<ethersBlockId>(ToEthers::cvt(&block)),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        let receipts = block
            .transactions
            .iter()
            .map(ToPrimitive::cvt)
            .map(|t| self.receipt_by_hash(t).unwrap().unwrap())
            .collect();
        Ok(Some(receipts))
    }
}

impl<P: JsonRpcClient> HeaderProvider for JsonRpcBcProvider<P> {
    #[doc = " Get header by block hash"]
    fn header(&self, block_hash: &BlockHash) -> rethResult<Option<Header>> {
        let block: Option<ethersBlock<ethersH256>> = self
            .runtime
            .block_on(
                self.provider
                    .get_block::<ethersBlockId>(ToEthers::cvt(block_hash)),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        let header = ToPrimitive::cvt(block);
        Ok(header.map(|h| h.header))
    }

    #[doc = " Get header by block number"]
    fn header_by_number(&self, num: u64) -> rethResult<Option<Header>> {
        let block_hash = self.block_hash(num)?;
        if block_hash.is_none() {
            return Ok(None);
        }
        let block_hash = block_hash.unwrap();
        self.header(&block_hash)
    }

    #[doc = " Get total difficulty by block hash."]
    fn header_td(&self, hash: &BlockHash) -> rethResult<Option<U256>> {
        let block: Option<ethersBlock<ethersH256>> = self
            .runtime
            .block_on(
                self.provider
                    .get_block::<ethersBlockId>(ToEthers::cvt(hash)),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        Ok(block.map(|b| ToPrimitive::cvt(&b.total_difficulty.unwrap())))
    }

    #[doc = " Get total difficulty by block number."]
    fn header_td_by_number(
        &self,
        number: BlockNumber,
    ) -> rethResult<Option<U256>> {
        let block: Option<ethersBlock<ethersH256>> = self
            .runtime
            .block_on(
                self.provider
                    .get_block::<ethersBlockId>(ToEthers::cvt(&number)),
            )
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        Ok(block.map(|b| ToPrimitive::cvt(&b.total_difficulty.unwrap())))
    }

    #[doc = " Get headers in range of block numbers"]
    fn headers_range(
        &self,
        range: impl RangeBounds<BlockNumber>,
    ) -> rethResult<Vec<Header>> {
        let mut bs = Vec::new();
        for bn in ToIterator::cvt(range) {
            let b = self.header_by_number(bn)?;
            if b.is_none() {
                break;
            }
            let b = b.unwrap();
            bs.push(b);
        }
        Ok(bs)
    }

    #[doc = " Get headers in range of block numbers"]
    fn sealed_headers_range(
        &self,
        range: impl RangeBounds<BlockNumber>,
    ) -> rethResult<Vec<SealedHeader>> {
        let mut bs = Vec::new();
        for bn in ToIterator::cvt(range) {
            let b = self.sealed_header(bn)?;
            if b.is_none() {
                break;
            }
            let b = b.unwrap();
            bs.push(b);
        }
        todo!()
    }

    #[doc = " Get a single sealed header by block number"]
    fn sealed_header(
        &self,
        number: BlockNumber,
    ) -> rethResult<Option<SealedHeader>> {
        let number = ethersBlockId::from(number);
        let block: Option<ethersBlock<ethersH256>> = self
            .runtime
            .block_on(self.provider.get_block(number))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        Ok(ToPrimitive::cvt(block))
    }
}

fn chain_id_to_chain_spec(id: u64) -> ChainSpec {
    match id {
        1 => ChainSpecBuilder::mainnet().build(),
        _ => panic!("Unsupported chain id: {}", id),
    }
}

impl<P: JsonRpcClient> EvmEnvProvider for JsonRpcBcProvider<P> {
    #[doc = " Fills the [CfgEnv] and [BlockEnv] fields with values specific to the given"]
    #[doc = " [BlockHashOrNumber]."]
    fn fill_env_at(
        &self,
        cfg: &mut CfgEnv,
        block_env: &mut BlockEnv,
        at: BlockHashOrNumber,
    ) -> rethResult<()> {
        let header = match at {
            BlockHashOrNumber::Hash(h) => self.header(&h)?,
            BlockHashOrNumber::Number(n) => self.header_by_number(n)?,
        };
        if header.is_none() {
            return Err(rethError::Provider(ProviderError::HeaderNotFound(at)));
        }
        let header = header.unwrap();
        self.fill_env_with_header(cfg, block_env, &header)
    }

    #[doc = " Fills the [CfgEnv] and [BlockEnv]  fields with values specific to the given [Header]."]
    fn fill_env_with_header(
        &self,
        cfg: &mut CfgEnv,
        block_env: &mut BlockEnv,
        header: &Header,
    ) -> rethResult<()> {
        self.fill_cfg_env_with_header(cfg, header)?;
        self.fill_block_env_with_header(block_env, header)?;
        Ok(())
    }

    #[doc = " Fills the [BlockEnv] fields with values specific to the given [BlockHashOrNumber]."]
    fn fill_block_env_at(
        &self,
        block_env: &mut BlockEnv,
        at: BlockHashOrNumber,
    ) -> rethResult<()> {
        let header = match at {
            BlockHashOrNumber::Hash(h) => self.header(&h)?,
            BlockHashOrNumber::Number(n) => self.header_by_number(n)?,
        };
        if header.is_none() {
            return Err(rethError::Provider(ProviderError::HeaderNotFound(at)));
        }
        let header = header.unwrap();
        self.fill_block_env_with_header(block_env, &header)
    }

    #[doc = " Fills the [BlockEnv] fields with values specific to the given [Header]."]
    fn fill_block_env_with_header(
        &self,
        block_env: &mut BlockEnv,
        header: &Header,
    ) -> rethResult<()> {
        let chain_id = self
            .runtime
            .block_on(self.provider.get_chainid())
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?
            .as_u64();
        let chain_spec = chain_id_to_chain_spec(chain_id);
        let after_merge =
            if chain_spec.paris_block_and_final_difficulty.is_none() {
                false
            } else {
                header.number
                    >= chain_spec.paris_block_and_final_difficulty.unwrap().0
            };
        reth_revm::env::fill_block_env(
            block_env,
            &chain_spec,
            header,
            after_merge,
        );
        Ok(())
    }

    #[doc = " Fills the [CfgEnv] fields with values specific to the given [BlockHashOrNumber]."]
    fn fill_cfg_env_at(
        &self,
        cfg: &mut CfgEnv,
        at: BlockHashOrNumber,
    ) -> rethResult<()> {
        let header = match at {
            BlockHashOrNumber::Hash(hash) => self.header(&hash)?,
            BlockHashOrNumber::Number(number) => {
                self.header_by_number(number)?
            }
        };
        if header.is_none() {
            return Err(rethError::Provider(ProviderError::HeaderNotFound(at)));
        }
        let header = header.unwrap();
        self.fill_cfg_env_with_header(cfg, &header)
    }

    #[doc = " Fills the [CfgEnv] fields with values specific to the given [Header]."]
    fn fill_cfg_env_with_header(
        &self,
        cfg: &mut CfgEnv,
        header: &Header,
    ) -> rethResult<()> {
        let chain_id = self
            .runtime
            .block_on(self.provider.get_chainid())
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?
            .as_u64();
        let chain_spec = chain_id_to_chain_spec(chain_id);
        reth_revm::env::fill_cfg_env(
            cfg,
            &chain_spec,
            header,
            header.difficulty,
        );
        Ok(())
    }
}

impl<P: JsonRpcClient> BlockIdProvider for JsonRpcBcProvider<P> {
    #[doc = " Get the current pending block number and hash."]
    fn pending_block_num_hash(
        &self,
    ) -> rethResult<Option<reth_primitives::BlockNumHash>> {
        self.finalized_block_num_hash()
    }

    #[doc = " Get the current safe block number and hash."]
    fn safe_block_num_hash(
        &self,
    ) -> rethResult<Option<reth_primitives::BlockNumHash>> {
        self.finalized_block_num_hash()
    }

    #[doc = " Get the current finalized block number and hash."]
    fn finalized_block_num_hash(
        &self,
    ) -> rethResult<Option<reth_primitives::BlockNumHash>> {
        let bn = self.last_block_number()?;
        let hash = self.block_hash(bn)?.unwrap();
        Ok(Some(reth_primitives::BlockNumHash { number: bn, hash }))
    }
}

impl<P: JsonRpcClient> StateProviderFactory for JsonRpcBcProvider<P> {
    fn latest(&self) -> rethResult<reth_provider::StateProviderBox<'_>> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        Ok(Box::new(JsonRpcStateProvider {
            runtime,
            provider: self.provider.clone(),
            at: None,
        }))
    }

    fn history_by_block_number(
        &self,
        block: BlockNumber,
    ) -> rethResult<reth_provider::StateProviderBox<'_>> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        Ok(Box::new(JsonRpcStateProvider {
            runtime,
            provider: self.provider.clone(),
            at: Some(block),
        }))
    }

    fn history_by_block_hash(
        &self,
        block: BlockHash,
    ) -> rethResult<reth_provider::StateProviderBox<'_>> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let block = self.block_number(block)?.unwrap();
        Ok(Box::new(JsonRpcStateProvider {
            runtime,
            provider: self.provider.clone(),
            at: Some(block),
        }))
    }

    fn state_by_block_hash(
        &self,
        block: BlockHash,
    ) -> rethResult<reth_provider::StateProviderBox<'_>> {
        self.history_by_block_hash(block)
    }

    fn pending(&self) -> rethResult<reth_provider::StateProviderBox<'_>> {
        todo!()
    }

    fn pending_with_provider(
        &self,
        _post_state_data: Box<dyn reth_provider::PostStateDataProvider>,
    ) -> rethResult<reth_provider::StateProviderBox<'_>> {
        todo!()
    }

    fn pending_state_by_hash(
        &self,
        _block_hash: H256,
    ) -> rethResult<Option<reth_provider::StateProviderBox<'_>>> {
        todo!()
    }
}

/// Global map from code hash to code.
/// This global mapping is possible because we assume unique code hash must map to unique code.
static CODE_HASH_TO_CODE: Mutex<Option<Arc<Mutex<HashMap<H256, Bytecode>>>>> =
    Mutex::new(None);

fn get_code_hash_map() -> Arc<Mutex<HashMap<H256, Bytecode>>> {
    let mut maybe_map = CODE_HASH_TO_CODE.lock().unwrap();
    if maybe_map.is_none() {
        let new_map = Arc::new(Mutex::new(HashMap::new()));
        *maybe_map = Some(new_map);
    }
    maybe_map.as_ref().unwrap().clone()
}

struct JsonRpcStateProvider<P> {
    runtime: tokio::runtime::Runtime,
    provider: Arc<Provider<P>>,
    at: Option<BlockNumber>,
}

impl<P: JsonRpcClient> JsonRpcStateProvider<P> {
    fn get_ethers_block_id(&self) -> Option<ethersBlockId> {
        self.at.map(|n| ToEthers::cvt(&n))
    }
}

impl<P: JsonRpcClient> BlockHashProvider for JsonRpcStateProvider<P> {
    #[doc = " Get the hash of the block with the given number. Returns `None` if no block with this number"]
    #[doc = " exists."]
    fn block_hash(&self, number: BlockNumber) -> rethResult<Option<H256>> {
        if let Some(at) = self.at {
            if number > at {
                return Ok(None);
            }
        }
        let h = self
            .runtime
            .block_on(self.provider.get_block(number))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?
            .and_then(|b| b.hash)
            .map(|h| h.0.into());
        Ok(h)
    }

    #[doc = " Get headers in range of block hashes or numbers"]
    fn canonical_hashes_range(
        &self,
        start: BlockNumber,
        end: BlockNumber,
    ) -> rethResult<Vec<H256>> {
        let mut hashes = Vec::new();
        for i in start..end {
            if let Some(h) = self.block_hash(i)? {
                hashes.push(h);
            } else {
                break;
            }
        }
        Ok(hashes)
    }
}

impl<P: JsonRpcClient> AccountReader for JsonRpcStateProvider<P> {
    #[doc = " Get basic account information."]
    #[doc = ""]
    #[doc = " Returns `None` if the account doesn\'t exist."]
    fn basic_account(&self, address: Address) -> rethResult<Option<Account>> {
        let nonce = self
            .runtime
            .block_on(self.provider.get_transaction_count(
                ToEthers::cvt(&address),
                self.get_ethers_block_id(),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        let balance = self
            .runtime
            .block_on(self.provider.get_balance(
                ToEthers::cvt(&address),
                self.get_ethers_block_id(),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        let code =
            self.runtime
                .block_on(self.provider.get_code(
                    ToEthers::cvt(&address),
                    self.get_ethers_block_id(),
                ))
                .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        let code_hash;
        if code.len() == 0 {
            code_hash = None;
        } else {
            let code: &[u8] = code.0.as_ref();
            let hash = reth_primitives::keccak256(code);
            code_hash = Some(hash);
            let code_hash_map = get_code_hash_map();
            code_hash_map
                .lock()
                .unwrap()
                .insert(hash, ToPrimitive::cvt(code));
        }
        Ok(Some(Account {
            nonce: nonce.as_u64(),
            balance: ToPrimitive::cvt(&balance),
            bytecode_hash: code_hash,
        }))
    }
}

impl<P: JsonRpcClient> StateRootProvider for JsonRpcStateProvider<P> {
    #[doc = " Returns the state root of the PostState on top of the current state."]
    fn state_root(&self, _post_state: PostState) -> rethResult<H256> {
        Err(rethError::Provider(
            ProviderError::StateRootNotAvailableForHistoricalBlock,
        ))
    }
}

impl<P: JsonRpcClient> StateProvider for JsonRpcStateProvider<P> {
    #[doc = " Get storage of given account."]
    fn storage(
        &self,
        account: Address,
        storage_key: StorageKey,
    ) -> rethResult<Option<StorageValue>> {
        let value = self
            .runtime
            .block_on(self.provider.get_storage_at(
                ToEthers::cvt(&account),
                ToEthers::cvt(&storage_key),
                self.get_ethers_block_id(),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        Ok(Some(U256::from_be_bytes(value.into())))
    }

    #[doc = " Get account code by its hash"]
    fn bytecode_by_hash(
        &self,
        code_hash: H256,
    ) -> rethResult<Option<Bytecode>> {
        // FIXME: this implmenetation assumes that get_code (fn basic_account) is called previously for the account
        // with code_hash.
        // This is due to the limitation that we cannot directly get code from code hash from
        // JSON-RPC.
        // If get_code is not called previously, this method will return None, even if the account
        // may have code on the blockchain.
        let code_hash_map = get_code_hash_map();
        let binding = code_hash_map.lock().unwrap();
        let code = binding.get(&code_hash);
        let code = code.cloned();
        Ok(code)
    }

    #[doc = " Get account and storage proofs."]
    fn proof(
        &self,
        _address: Address,
        _keys: &[H256],
    ) -> rethResult<(Vec<Bytes>, H256, Vec<Vec<Bytes>>)> {
        Err(rethError::Provider(
            ProviderError::StateRootNotAvailableForHistoricalBlock,
        ))
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {

    use ethers_providers::Http;
    use reth_provider::BlockNumProvider;

    use crate::{
        config::flags::SoflConfig, engine::providers::BcProviderBuilder,
    };

    use super::JsonRpcBcProvider;

    fn get_bc_provider() -> JsonRpcBcProvider<Http> {
        let cfg = SoflConfig::load().unwrap();
        let url = cfg.jsonrpc.endpoint.clone();
        BcProviderBuilder::with_jsonrpc_via_http_with_auth(url, cfg.jsonrpc)
            .unwrap()
    }

    #[test]
    fn test_connection() {
        let provider = get_bc_provider();
        let bn = provider.last_block_number().unwrap();
        assert!(bn > 0);
    }

    mod tests_transactions_provider {
        use std::ops::Range;

        use reth_provider::TransactionsProvider;

        use crate::engine::providers::rpc::tests_with_jsonrpc::get_bc_provider;

        #[test]
        fn test_get_block_txs() {
            let provider = get_bc_provider();
            let range = Range {
                start: 14000000,
                end: 14000003,
            };
            let block_txs = provider.transactions_by_block_range(range);
            assert!(block_txs.is_ok());
            let block_txs = block_txs.unwrap();
            assert_eq!(block_txs.len(), 3);
            assert_eq!(block_txs[0].len(), 112);
            assert_eq!(block_txs[1].len(), 33);
            assert_eq!(block_txs[2].len(), 335);
        }
    }

    mod tests_header_provider {
        use reth_provider::HeaderProvider;
        use revm_primitives::hex;

        use crate::engine::providers::rpc::tests_with_jsonrpc::get_bc_provider;

        #[test]
        fn test_get_header() {
            let provider = get_bc_provider();
            let sealed_header = provider.sealed_header(14000000).unwrap();
            assert!(sealed_header.is_some());
            let sealed_header = sealed_header.unwrap();
            assert_eq!(sealed_header.number, 14000000);
            assert_eq!(
            hex::encode(sealed_header.hash.as_slice()),
            "9bff49171de27924fa958faf7b7ce605c1ff0fdee86f4c0c74239e6ae20d9446"
        );
            assert_eq!(sealed_header.gas_used, 8119826)
        }
    }

    mod test_evm_env_provider {
        use reth_provider::EvmEnvProvider;
        use revm_primitives::{BlockEnv, CfgEnv, U256};

        use super::get_bc_provider;

        #[test]
        fn test_fill_env_at() {
            let provider = get_bc_provider();
            let mut cfg = CfgEnv::default();
            let mut block_env = BlockEnv::default();
            let r =
                provider.fill_env_at(&mut cfg, &mut block_env, 14000000.into());
            assert!(r.is_ok());
            assert_eq!(cfg.chain_id, U256::from(1));
            assert_eq!(block_env.number, U256::from(14000000));
            assert_eq!(
                block_env.difficulty,
                U256::from_str_radix("12316581093827601", 10).unwrap()
            );
            assert_eq!(
                block_env.gas_limit,
                U256::from_str_radix("30058561", 10).unwrap()
            );
        }
    }

    mod test_state_provider {
        use reth_provider::StateProviderFactory;

        use crate::utils::conversion::{Convert, ToPrimitive};

        use super::get_bc_provider;

        #[test]
        fn test_get_latest_state_provider() {
            let bc_provider = get_bc_provider();
            let state_provider = bc_provider.latest();
            assert!(state_provider.is_ok());
            let state_provider = state_provider.unwrap();
            let hash = state_provider.block_hash(17450000);
            assert!(hash.is_ok());
            assert!(hash.unwrap().is_some());
        }

        #[test]
        fn test_historical_state_provider_has_cutoff() {
            let bc_provider = get_bc_provider();
            let state_provider =
                bc_provider.history_by_block_number(16000000).unwrap();
            let hash = state_provider.block_hash(17450000);
            assert!(hash.is_ok());
            assert!(hash.unwrap().is_none());
        }

        #[test]
        fn test_state_provider_account_info() {
            let bc_provider = get_bc_provider();
            let state_provider =
                bc_provider.history_by_block_number(17000000).unwrap();
            let account = state_provider.basic_account(ToPrimitive::cvt(
                "0x690B9A9E9aa1C9dB991C7721a92d351Db4FaC990",
            ));
            assert!(account.is_ok());
            let account = account.unwrap();
            assert!(account.is_some());
            let account = account.unwrap();
            assert_eq!(account.nonce, 246861);

            // Tether Contract
            let tether = state_provider.basic_account(ToPrimitive::cvt(
                "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            ));
            assert!(tether.is_ok());
            let tether = tether.unwrap();
            assert!(tether.is_some());
            let tether = tether.unwrap();
            let code_hash = tether.bytecode_hash;
            assert!(code_hash.is_some());
            let code_hash = code_hash.unwrap();
            assert_eq!(
                code_hash,
                ToPrimitive::cvt("0xb44fb4e949d0f78f87f79ee46428f23a2a5713ce6fc6e0beb3dda78c2ac1ea55")
            );
        }

        #[test]
        fn test_state_provider_storage() {
            let bc_provider = get_bc_provider();
            let state_provider =
                bc_provider.history_by_block_number(17000000).unwrap();
            // Test oracle based on transaction
            // 0x39cd4f06f5cb93f108bb53c4687e3048da9b38a494c30344f6a1ef3f413644cc
            let weth =
                ToPrimitive::cvt("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
            let slot = ToPrimitive::cvt("0x12231cd4c753cb5530a43a74c45106c24765e6f81dc8927d4f4be7e53315d5a8");
            let expected = ToPrimitive::cvt("0x000000000000000000000000000000000000000000000003617b7114e5ff3e79");
            let actual = state_provider.storage(weth, slot).unwrap();
            assert!(actual.is_some());
            let actual = actual.unwrap();
            assert_eq!(actual, expected);
        }
    }
}
