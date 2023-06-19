use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use ethers::providers::{Middleware, Provider};
use ethers::types::{
    Address as ethersAddress, Block as ethersBlock, BlockId as ethersBlockId,
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
    Account, Address, BlockHash, BlockHashOrNumber, BlockNumber, Bloom,
    Bytecode, Bytes, ChainInfo, ChainSpec, ChainSpecBuilder, Header,
    SealedHeader, StorageKey, StorageValue, TransactionMeta, TransactionSigned,
    TxHash, TxNumber,
};
use reth_provider::{
    AccountProvider, BlockHashProvider, BlockIdProvider, BlockNumProvider,
    EvmEnvProvider, HeaderProvider, PostState, ProviderError, StateProvider,
    StateProviderFactory, StateRootProvider, TransactionsProvider,
};
use reth_rlp::Decodable;
use revm_primitives::{BlockEnv, CfgEnv, B256 as H256, U256};

use crate::utils::conversion::{Convert, FromEthers, ToEthers, ToIterator};

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
        let tx = FromEthers::cvt(&tx);
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
        let txs = block
            .transactions
            .iter()
            .map(|tx| FromEthers::cvt(tx))
            .collect();
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
        let header = FromEthers::cvt(block);
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
        Ok(block.map(|b| FromEthers::cvt(b.total_difficulty.unwrap())))
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
        Ok(block.map(|b| FromEthers::cvt(b.total_difficulty.unwrap())))
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
        Ok(FromEthers::cvt(block))
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
        let _ = self.fill_cfg_env_with_header(cfg, header)?;
        let _ = self.fill_block_env_with_header(block_env, header)?;
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
        let after_merge;
        if chain_spec.paris_block_and_final_difficulty.is_none() {
            after_merge = false;
        } else {
            after_merge = header.number
                >= chain_spec.paris_block_and_final_difficulty.unwrap().0;
        }
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
            &header,
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

impl<P: JsonRpcClient> AccountProvider for JsonRpcStateProvider<P> {
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
            let code: &[u8] = &code.0.as_ref();
            let hash = reth_primitives::keccak256(code);
            code_hash = Some(hash);
        }
        Ok(Some(Account {
            nonce: nonce.as_u64(),
            balance: U256::from_be_bytes(balance.into()),
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
        _code_hash: H256,
    ) -> rethResult<Option<Bytecode>> {
        todo!()
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
    use std::ops::Range;

    use ethers_providers::Http;
    use reth_provider::HeaderProvider;
    use reth_provider::{BlockNumProvider, TransactionsProvider};
    use revm_primitives::hex;

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
}
