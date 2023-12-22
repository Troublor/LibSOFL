use std::{cell::RefCell, collections::HashMap};

use alloy_providers::provider::{Provider, TempProvider};
use alloy_rpc_types::{Block, BlockNumberOrTag};
use libsofl_core::{
    blockchain::{provider::BcProvider, transaction::Tx, tx_position::TxPosition},
    conversion::ConvertTo,
    engine::types::{
        AnalysisKind, BlobExcessGasAndPrice, BlockEnv, BlockHash, BlockHashOrNumber, BlockNumber,
        CfgEnv, SpecId, TxEnv, TxHashOrPosition,
    },
    error::SoflError,
};

use crate::blockchain::JsonRpcTx;

pub struct JsonRpcProvider {
    pub(crate) p: Provider,

    rt: tokio::runtime::Runtime,

    // caches
    chain_id: u64,
    txs: RefCell<HashMap<TxHashOrPosition, JsonRpcTx>>,
    txs_in_block: RefCell<HashMap<BlockHashOrNumber, Vec<JsonRpcTx>>>,
    block_by_hash: RefCell<HashMap<BlockHash, Block>>,
    block_by_number: RefCell<HashMap<BlockNumber, Block>>,
}

impl JsonRpcProvider {
    fn block(&self, block: BlockHashOrNumber) -> Result<Block, SoflError> {
        match block {
            BlockHashOrNumber::Hash(hash) => self
                .block_by_hash
                .borrow()
                .get(&hash)
                .map(|b| Result::<Block, SoflError>::Ok(b.clone()))
                .unwrap_or_else(|| {
                    let task = self.p.get_block_by_hash(hash, false);
                    let blk = self
                        .rt
                        .block_on(task)
                        .map_err(|e| SoflError::Provider(format!("{:?}", e)))?;
                    let blk = blk.ok_or(SoflError::NotFound(format!("block {}", hash)))?;
                    self.block_by_hash.borrow_mut().insert(hash, blk.clone());
                    Ok(blk)
                }),
            BlockHashOrNumber::Number(number) => self
                .block_by_number
                .borrow()
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
                    self.block_by_number
                        .borrow_mut()
                        .insert(number, blk.clone());
                    Ok(blk)
                }),
        }
    }
}

impl BcProvider<JsonRpcTx> for JsonRpcProvider {
    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn tx(&self, tx: TxHashOrPosition) -> Result<JsonRpcTx, SoflError> {
        self.txs
            .borrow()
            .get(&tx)
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
                self.txs.borrow_mut().insert(tx, t.clone());
                Ok(t)
            })
    }

    fn txs_in_block(&self, block: BlockHashOrNumber) -> Result<Vec<JsonRpcTx>, SoflError> {
        self.txs_in_block
            .borrow()
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
                self.txs_in_block.borrow_mut().insert(block, ts.clone());
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
