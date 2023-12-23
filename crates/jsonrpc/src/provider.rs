use std::{cell::RefCell, collections::HashMap};

use alloy_providers::provider::{Provider, TempProvider};
use alloy_rpc_types::{Block, BlockNumberOrTag};
use alloy_transport_http::Http;
use libsofl_core::{
    blockchain::{provider::BcProvider, transaction::Tx, tx_position::TxPosition},
    conversion::ConvertTo,
    engine::types::{
        AnalysisKind, BlobExcessGasAndPrice, BlockEnv, BlockHash, BlockHashOrNumber, BlockNumber,
        CfgEnv, SpecId, TxEnv, TxHashOrPosition,
    },
    error::SoflError,
};
use reqwest::Client;

use crate::blockchain::JsonRpcTx;

pub struct JsonRpcProvider {
    pub(crate) url: String,
    pub(crate) p: Provider<Http<Client>>,

    pub(crate) rt: tokio::runtime::Runtime,

    // caches
    pub(crate) chain_id: u64,
    pub(crate) txs: RefCell<HashMap<TxHashOrPosition, JsonRpcTx>>,
    pub(crate) txs_in_block: RefCell<HashMap<BlockHashOrNumber, Vec<JsonRpcTx>>>,
    pub(crate) block_by_hash: RefCell<HashMap<BlockHash, Block>>,
    pub(crate) block_by_number: RefCell<HashMap<BlockNumber, Block>>,
}

impl JsonRpcProvider {
    pub fn new(url: String) -> Result<JsonRpcProvider, SoflError> {
        let p = Provider::try_from(url.clone()).expect("failed to create jsonrpc provider");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        let chain_id = rt
            .block_on(p.get_chain_id())
            .map_err(|e| SoflError::Provider(format!("failed to get chain id: {:?}", e)))?;
        Ok(JsonRpcProvider {
            url,
            p,
            rt,
            chain_id: chain_id.cvt(),
            txs: Default::default(),
            txs_in_block: Default::default(),
            block_by_hash: Default::default(),
            block_by_number: Default::default(),
        })
    }
}

impl Clone for JsonRpcProvider {
    fn clone(&self) -> Self {
        let p = Provider::try_from(self.url.clone()).expect("failed to create jsonrpc provider");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        Self {
            url: self.url.clone(),
            p,
            rt,
            chain_id: self.chain_id,
            txs: self.txs.clone(),
            txs_in_block: self.txs_in_block.clone(),
            block_by_hash: self.block_by_hash.clone(),
            block_by_number: self.block_by_number.clone(),
        }
    }
}

impl JsonRpcProvider {
    fn block(&self, block: BlockHashOrNumber) -> Result<Block, SoflError> {
        match block {
            BlockHashOrNumber::Hash(hash) => {
                let mut block_by_hash = self.block_by_hash.borrow_mut();
                block_by_hash
                    .get(&hash)
                    .map(|b| Result::<Block, SoflError>::Ok(b.clone()))
                    .unwrap_or_else(|| {
                        let task = self.p.get_block_by_hash(hash, false);
                        let blk = self
                            .rt
                            .block_on(task)
                            .map_err(|e| SoflError::Provider(format!("{:?}", e)))?;
                        let blk = blk.ok_or(SoflError::NotFound(format!("block {}", hash)))?;
                        block_by_hash.insert(hash, blk.clone());
                        let bn: u64 = blk.header.number.expect("block number").cvt();
                        self.block_by_number.borrow_mut().insert(bn, blk.clone());
                        Ok(blk)
                    })
            }
            BlockHashOrNumber::Number(number) => {
                let mut block_by_number = self.block_by_number.borrow_mut();
                block_by_number
                    .get(&number)
                    .map(|b| Result::<Block, SoflError>::Ok(b.clone()))
                    .unwrap_or_else(|| {
                        let task = self
                            .p
                            .get_block_by_number(BlockNumberOrTag::Number(number), false);
                        let blk = self
                            .rt
                            .block_on(task)
                            .map_err(|e| SoflError::Provider(format!("{:?}", e)))?;
                        let blk = blk.ok_or(SoflError::NotFound(format!("block {}", number)))?;
                        block_by_number.insert(number, blk.clone());
                        let hash = blk.header.hash.expect("block hash");
                        self.block_by_hash.borrow_mut().insert(hash, blk.clone());
                        Ok(blk)
                    })
            }
        }
    }
}

impl BcProvider<JsonRpcTx> for JsonRpcProvider {
    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn tx(&self, tx: TxHashOrPosition) -> Result<JsonRpcTx, SoflError> {
        let mut txs = self.txs.borrow_mut();
        txs.get(&tx)
            .map(|t| Result::<JsonRpcTx, SoflError>::Ok(t.clone()))
            .unwrap_or_else(move || {
                let task = match &tx {
                    TxHashOrPosition::Hash(hash) => self.p.get_transaction_by_hash(*hash),
                    TxHashOrPosition::Position(TxPosition { block, index }) => {
                        let blk = self.block(*block)?;
                        let hash = blk.transactions.hashes().skip(*index as usize).next();
                        let hash = hash.ok_or(SoflError::NotFound(format!(
                            "transaction {} in block {}",
                            index, block
                        )))?;
                        self.p.get_transaction_by_hash(*hash)
                    }
                };
                let transaction = self.rt.block_on(task).map_err(|e| {
                    SoflError::Provider(format!("failed to get transaction {}: {:?}", tx, e))
                })?;
                let task = self.p.get_transaction_receipt(transaction.hash);
                let receipt = self.rt.block_on(task).map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to get transaction {} receipt: {:?}",
                        transaction.hash, e
                    ))
                })?;
                let t = JsonRpcTx {
                    tx: transaction,
                    receipt,
                };
                txs.insert(tx, t.clone());
                Ok(t)
            })
    }

    fn txs_in_block(&self, block: BlockHashOrNumber) -> Result<Vec<JsonRpcTx>, SoflError> {
        let mut txs_in_block = self.txs_in_block.borrow_mut();
        txs_in_block
            .get(&block)
            .map(|ts| Result::<Vec<JsonRpcTx>, SoflError>::Ok(ts.clone()))
            .unwrap_or_else(|| {
                let blk = self.block(block)?;
                let ts: Result<Vec<JsonRpcTx>, SoflError> = blk
                    .transactions
                    .hashes()
                    .map(|h| self.tx(TxHashOrPosition::Hash(*h)))
                    .collect();
                let ts = ts?;
                txs_in_block.insert(block, ts.clone());
                Ok(ts)
            })
    }

    fn block_number_by_hash(&self, hash: BlockHash) -> Result<BlockNumber, SoflError> {
        self.block(BlockHashOrNumber::Hash(hash))?
            .header
            .number
            .map(|n| n.cvt())
            .ok_or(SoflError::NotFound(format!(
                "block number by hash {}",
                hash
            )))
    }

    fn block_hash_by_number(&self, number: BlockNumber) -> Result<BlockHash, SoflError> {
        self.block(BlockHashOrNumber::Number(number))?
            .header
            .hash
            .ok_or(SoflError::NotFound(format!(
                "block hash by number {}",
                number
            )))
    }
    fn fill_cfg_env(&self, env: &mut CfgEnv, block: BlockHashOrNumber) -> Result<(), SoflError> {
        let number = match block {
            BlockHashOrNumber::Hash(hash) => self.block_number_by_hash(hash)?,
            BlockHashOrNumber::Number(number) => number,
        };
        env.chain_id = self.chain_id;
        env.perf_analyse_created_bytecodes = AnalysisKind::Analyse;
        env.spec_id = match number {
            0..=199999 => SpecId::FRONTIER,
            200000..=1149999 => SpecId::FRONTIER_THAWING,
            1150000..=1919999 => SpecId::HOMESTEAD,
            1920000..=2462999 => SpecId::DAO_FORK,
            2463000..=2674999 => SpecId::TANGERINE,
            2675000..=4369999 => SpecId::SPURIOUS_DRAGON,
            4370000..=7279999 => SpecId::BYZANTIUM,
            // 7280000..9069000 => SpecId::CONSTANTINOPLE,
            7280000..=9068999 => SpecId::PETERSBURG,
            9069000..=9199999 => SpecId::ISTANBUL,
            9200000..=12243999 => SpecId::MUIR_GLACIER,
            12244000..=12964999 => SpecId::BERLIN,
            12965000..=13772999 => SpecId::LONDON,
            13773000..=15049999 => SpecId::ARROW_GLACIER,
            15050000..=15537393 => SpecId::GRAY_GLACIER,
            15537394..=17034869 => SpecId::MERGE,
            17034870.. => SpecId::SHANGHAI,
        };
        Ok(())
    }

    fn fill_block_env(
        &self,
        env: &mut BlockEnv,
        block: BlockHashOrNumber,
    ) -> Result<(), SoflError> {
        let header = self.block(block)?.header;
        env.number = header.number.ok_or(SoflError::NotFound(format!(
            "block number not available {}",
            block
        )))?;
        env.coinbase = header.miner;
        env.timestamp = header.timestamp;
        env.gas_limit = header.gas_limit;
        env.basefee = header.base_fee_per_gas.unwrap_or_default();
        env.difficulty = header.difficulty;
        env.prevrandao = header.mix_hash;
        env.blob_excess_gas_and_price = header
            .excess_blob_gas
            .map(|g| BlobExcessGasAndPrice::new(g.cvt()));
        Ok(())
    }

    fn fill_tx_env(&self, env: &mut TxEnv, tx: TxHashOrPosition) -> Result<(), SoflError> {
        let tx = self.tx(tx)?;
        tx.fill_tx_env(env)
    }
}

#[cfg(test)]
mod tests {
    use libsofl_core::{
        blockchain::{provider::BcProvider, transaction::Tx},
        conversion::ConvertTo,
    };

    use crate::config::JsonRpcConfig;

    #[test]
    fn test_chain_id() {
        let bp = JsonRpcConfig::must_load().bc_provider().unwrap();
        let id = bp.chain_id();
        assert!(id > 0);
    }

    #[test]
    fn test_block_txs() {
        let bp = JsonRpcConfig::must_load().bc_provider().unwrap();
        let txs = bp.txs_in_block(100004u64.cvt()).unwrap();
        assert_eq!(txs.len(), 1);
        let tx = &txs[0];
        assert_eq!(
            tx.hash().to_string(),
            "0x6f12399cc2cb42bed5b267899b08a847552e8c42a64f5eb128c1bcbd1974fb0c"
        );
    }

    #[test]
    fn test_tx() {
        let bp = JsonRpcConfig::must_load().bc_provider().unwrap();
        let tx = bp
            .tx("0x8b0fb47fa601051c292a5dce9b4a4e94b62d5cb9e58ebd4fad3febb735fa131c".cvt())
            .unwrap();
        assert_eq!(
            ConvertTo::<String>::cvt(&tx.sender()),
            "0x69947af8f3E8e5f338DBB7Ea3d4b1C573e18fF58"
        );
        assert_eq!(
            ConvertTo::<String>::cvt(&tx.to().unwrap()),
            "0x0c2E57EFddbA8c768147D1fdF9176a0A6EBd5d83"
        );
    }
}
