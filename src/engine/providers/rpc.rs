use std::any::Any;
use std::collections::HashMap;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use ethers::providers::{Middleware, Provider};
use ethers::types::{
    Address as ethersAddress, Block as ethersBlock, BlockId as ethersBlockId,
    Transaction as ethersTransaction,
    TransactionReceipt as ethersTransactionReceipt, TxHash as ethersTxHash,
    H256 as ethersH256, U256 as ethersU256,
};
use ethers_providers::Http;
use ethers_providers::JsonRpcClient;
use futures::executor::block_on;
use futures::future::join_all;
use futures::StreamExt;
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
use tokio::runtime::Runtime;

use super::{BcProvider, BcProviderBuilder};
use futures::join;

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

    // TODO: websocket support
}

pub struct JsonRpcBcProvider<P: JsonRpcClient> {
    pub(crate) provider: Arc<Provider<P>>,
    runtime: tokio::runtime::Runtime,
}

impl<P: JsonRpcClient> BlockHashProvider for JsonRpcBcProvider<P> {
    #[doc = " Get the hash of the block with the given number. Returns `None` if no block with this number"]
    #[doc = " exists."]
    fn block_hash(&self, number: BlockNumber) -> rethResult<Option<H256>> {
        let h = block_on(self.provider.get_block(number))
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
        let bs = block_on(join_all(fs));
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
        let h = block_on(
            self.provider
                .get_block(ethersTxHash::from_slice(hash.as_bytes())),
        )
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
    fn transaction_id(&self, tx_hash: TxHash) -> rethResult<Option<TxNumber>> {
        todo!()
    }

    #[doc = " Get transaction by id."]
    fn transaction_by_id(
        &self,
        id: TxNumber,
    ) -> rethResult<Option<TransactionSigned>> {
        todo!()
    }

    #[doc = " Get transaction by transaction hash."]
    fn transaction_by_hash(
        &self,
        hash: TxHash,
    ) -> rethResult<Option<TransactionSigned>> {
        let tx = block_on(
            self.provider
                .get_transaction(ethersTxHash::from_slice(hash.as_bytes())),
        )
        .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if tx.is_none() {
            return Ok(None);
        }
        let tx = tx.unwrap();
        let rlp = tx.rlp();
        let mut rlp = rlp.as_ref();
        let tx = TransactionSigned::decode(&mut rlp).unwrap();
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
        let receipt: Option<ethersTransactionReceipt> =
            block_on(self.provider.get_transaction_receipt(
                ethersTxHash::from_slice(hash.as_bytes()),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if receipt.is_none() {
            return Ok(None);
        }
        let receipt = receipt.unwrap();
        let block: Option<ethersBlock<ethersTxHash>> =
            block_on(self.provider.get_block(receipt.block_hash.unwrap()))
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
        id: TxNumber,
    ) -> rethResult<Option<BlockNumber>> {
        todo!()
    }

    #[doc = " Get transactions by block id."]
    fn transactions_by_block(
        &self,
        block: BlockHashOrNumber,
    ) -> rethResult<Option<Vec<TransactionSigned>>> {
        let block_id: ethersBlockId = match block {
            BlockHashOrNumber::Hash(h) => {
                ethersBlockId::Hash(ethersTxHash::from_slice(h.as_bytes()))
            }
            BlockHashOrNumber::Number(n) => ethersBlockId::Number(n.into()),
        };
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
            .map(|tx| {
                let rlp = tx.rlp();
                let mut rlp = rlp.as_ref();
                TransactionSigned::decode(&mut rlp).unwrap()
            })
            .collect();
        Ok(Some(txs))
    }

    #[doc = " Get transactions by block range."]
    fn transactions_by_block_range(
        &self,
        range: impl RangeBounds<BlockNumber>,
    ) -> rethResult<Vec<Vec<TransactionSigned>>> {
        let start = match range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(n) => n + 1,
            Bound::Excluded(n) => *n,
            Bound::Unbounded => self.last_block_number()? + 1,
        };
        let mut bs = Vec::new();
        for bn in start..end {
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

fn convert_ethers_block_to_sealed_header(
    block: ethersBlock<ethersTxHash>,
) -> Option<SealedHeader> {
    if block.author.is_none() {
        // return None if the block is still pending
        return None;
    }
    let header = Header {
        parent_hash: H256::from_slice(block.parent_hash.as_bytes()),
        ommers_hash: H256::from_slice(block.uncles_hash.as_bytes()),
        beneficiary: Address::from_slice(block.author.unwrap().as_bytes()),
        state_root: H256::from_slice(block.state_root.as_bytes()),
        transactions_root: H256::from_slice(block.transactions_root.as_bytes()),
        receipts_root: H256::from_slice(block.receipts_root.as_bytes()),
        withdrawals_root: block
            .withdrawals_root
            .map(|r| H256::from_slice(r.as_bytes())),
        logs_bloom: Bloom::from_slice(block.logs_bloom.unwrap().as_bytes()),
        difficulty: U256::from_be_bytes(block.difficulty.into()),
        number: block.number.unwrap().as_u64(),
        gas_limit: block.gas_limit.as_u64(),
        gas_used: block.gas_used.as_u64(),
        timestamp: block.timestamp.as_u64(),
        mix_hash: H256::from_slice(block.mix_hash.unwrap().as_bytes()),
        nonce: block.nonce.unwrap().to_low_u64_be(), // TODO: check whether big-endian is
        // correct
        base_fee_per_gas: block.base_fee_per_gas.map(|f| f.as_u64()),
        extra_data: block.extra_data.0.into(),
    };
    let hash = block.hash.unwrap().0.into();
    Some(SealedHeader { header, hash })
}
impl<P: JsonRpcClient> HeaderProvider for JsonRpcBcProvider<P> {
    #[doc = " Get header by block hash"]
    fn header(&self, block_hash: &BlockHash) -> rethResult<Option<Header>> {
        let hash = block_hash.as_slice();
        let hash = ethersH256::from_slice(hash);
        let block: Option<ethersBlock<ethersH256>> =
            block_on(self.provider.get_block(ethersBlockId::from(hash)))
                .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        let header = convert_ethers_block_to_sealed_header(block);
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
        let block = self.header(hash)?;
        Ok(block.map(|b| b.difficulty))
    }

    #[doc = " Get total difficulty by block number."]
    fn header_td_by_number(
        &self,
        number: BlockNumber,
    ) -> rethResult<Option<U256>> {
        let block = self.header_by_number(number)?;
        Ok(block.map(|b| b.difficulty))
    }

    #[doc = " Get headers in range of block numbers"]
    fn headers_range(
        &self,
        range: impl RangeBounds<BlockNumber>,
    ) -> rethResult<Vec<Header>> {
        let start = match range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(n) => n + 1,
            Bound::Excluded(n) => *n,
            Bound::Unbounded => self.last_block_number()? + 1,
        };
        let mut bs = Vec::new();
        for bn in start..end {
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
        let start = match range.start_bound() {
            Bound::Included(n) => *n,
            Bound::Excluded(n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(n) => n + 1,
            Bound::Excluded(n) => *n,
            Bound::Unbounded => self.last_block_number()? + 1,
        };
        let mut bs = Vec::new();
        for bn in start..end {
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
        let block: Option<ethersBlock<ethersH256>> =
            block_on(self.provider.get_block(number))
                .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        if block.is_none() {
            return Ok(None);
        }
        let block = block.unwrap();
        Ok(convert_ethers_block_to_sealed_header(block))
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
        post_state_data: Box<dyn reth_provider::PostStateDataProvider>,
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
        self.at.map(|n| ethersBlockId::from(n))
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
        let h = block_on(self.provider.get_block(number))
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
                ethersAddress::from_slice(address.as_slice()),
                self.get_ethers_block_id(),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        let balance = self
            .runtime
            .block_on(self.provider.get_balance(
                ethersAddress::from_slice(address.as_slice()),
                self.get_ethers_block_id(),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        let code = self
            .runtime
            .block_on(self.provider.get_code(
                ethersAddress::from_slice(address.as_slice()),
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
    fn state_root(&self, post_state: PostState) -> rethResult<H256> {
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
                ethersAddress::from_slice(account.as_slice()),
                storage_key.into(),
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
        let code = self
            .runtime
            .block_on(self.provider.get_code(
                ethersAddress::from_slice(code_hash.as_bytes()),
                self.get_ethers_block_id(),
            ))
            .map_err(|_| rethError::Network(NetworkError::ChannelClosed))?;
        Ok(Some(Bytecode::new_raw(code.0)))
    }

    #[doc = " Get account and storage proofs."]
    fn proof(
        &self,
        address: Address,
        keys: &[H256],
    ) -> rethResult<(Vec<Bytes>, H256, Vec<Vec<Bytes>>)> {
        Err(rethError::Provider(
            ProviderError::StateRootNotAvailableForHistoricalBlock,
        ))
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use std::ops::Range;

    use ethers_providers::{Http, Middleware};
    use reth_provider::{BlockNumProvider, TransactionsProvider};

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
