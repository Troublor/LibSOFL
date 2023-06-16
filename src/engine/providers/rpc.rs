use std::ops::{Bound, RangeBounds};

use ethers::providers::{Middleware, Provider};
use ethers::types::{
    Block as ethersBlock, BlockId as ethersBlockId, Transaction as ethersTransaction,
    TransactionReceipt as ethersTransactionReceipt, TxHash as ethersTxHash,
};
use ethers_providers::Http;

use ethers_providers::JsonRpcClient;
use futures::executor::block_on;
use futures::future::join_all;
use futures::StreamExt;
use reth_interfaces::Error as rethError;
use reth_interfaces::Result as rethResult;
use reth_network_api::NetworkError;
use reth_primitives::{
    BlockHashOrNumber, BlockNumber, ChainInfo, TransactionMeta, TransactionSigned, TxHash, TxNumber,
};
use reth_provider::{BlockHashProvider, BlockNumProvider, ProviderError, TransactionsProvider};
use reth_rlp::Decodable;
use revm_primitives::B256 as H256;
use tokio::runtime::Runtime;

use super::{BcProvider, BcProviderBuilder};
use futures::join;

#[derive(Debug, Clone)]
pub enum JsonRpcError {
    InvalidEndpoint(String),
}

impl BcProviderBuilder {
    pub fn with_jsonrpc_via_http(url: String) -> Result<JsonRpcBcProvider<Http>, JsonRpcError> {
        // TODO: use retry client
        let provider =
            Provider::<Http>::try_from(&url).map_err(|_| JsonRpcError::InvalidEndpoint(url))?;
        let mut runtime = tokio::runtime::Runtime::new().unwrap();
        Ok(JsonRpcBcProvider { provider, runtime })
    }

    // TODO: websocket support
}

pub struct JsonRpcBcProvider<P: JsonRpcClient> {
    pub(crate) provider: Provider<P>,
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
            let b: rethResult<Option<ethersBlock<ethersTxHash>>> = self
                .provider
                .get_block(n)
                .await
                .map_err(|_| rethError::Network(NetworkError::ChannelClosed));
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
    fn transaction_by_id(&self, id: TxNumber) -> rethResult<Option<TransactionSigned>> {
        todo!()
    }

    #[doc = " Get transaction by transaction hash."]
    fn transaction_by_hash(&self, hash: TxHash) -> rethResult<Option<TransactionSigned>> {
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
        let receipt: Option<ethersTransactionReceipt> = block_on(
            self.provider
                .get_transaction_receipt(ethersTxHash::from_slice(hash.as_bytes())),
        )
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
    fn transaction_block(&self, id: TxNumber) -> rethResult<Option<BlockNumber>> {
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
            let b = self.transactions_by_block(BlockHashOrNumber::Number(bn))?;
            if b.is_none() {
                break;
            }
            let b = b.unwrap();
            bs.push(b);
        }
        Ok(bs)
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use std::ops::Range;

    use ethers_providers::{Http, Middleware};
    use reth_provider::{BlockNumProvider, TransactionsProvider};

    use crate::{config::flags::SoflConfig, engine::providers::BcProviderBuilder};

    use super::JsonRpcBcProvider;

    fn get_bc_provider() -> JsonRpcBcProvider<Http> {
        let cfg = SoflConfig::load().unwrap();
        let url = cfg.jsonrpc.endpoint;
        BcProviderBuilder::with_jsonrpc_via_http(url).unwrap()
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
