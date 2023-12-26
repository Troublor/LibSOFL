use auto_impl::auto_impl;
use mockall::automock;
use revm::DatabaseRef;

use crate::engine::memory::MemoryBcState;
use crate::engine::types::BlockEnv;
use crate::engine::types::BlockHash;
use crate::engine::types::BlockHashOrNumber;
use crate::engine::types::BlockNumber;
use crate::engine::types::CfgEnv;
use crate::engine::types::TxEnv;
use crate::engine::types::TxHashOrPosition;
use crate::error::SoflError;

use super::transaction::Tx;
use super::tx_position::TxPosition;

#[auto_impl(&, Box)]
#[automock]
pub trait BcProvider<T: Tx> {
    // chain info
    fn chain_id(&self) -> u64;

    // transaction information
    fn tx(&self, tx: TxHashOrPosition) -> Result<T, SoflError>;
    fn txs_in_block(
        &self,
        block: BlockHashOrNumber,
    ) -> Result<Vec<T>, SoflError>;

    // block info
    fn block_number_by_hash(
        &self,
        hash: BlockHash,
    ) -> Result<BlockNumber, SoflError>;
    fn block_hash_by_number(
        &self,
        number: BlockNumber,
    ) -> Result<BlockHash, SoflError>;

    // revm env filler
    fn fill_cfg_env(
        &self,
        env: &mut CfgEnv,
        block: BlockHashOrNumber,
    ) -> Result<(), SoflError>;
    fn fill_block_env(
        &self,
        env: &mut BlockEnv,
        block: BlockHashOrNumber,
    ) -> Result<(), SoflError>;
    fn fill_tx_env(
        &self,
        env: &mut TxEnv,
        tx: TxHashOrPosition,
    ) -> Result<(), SoflError>;
}

#[auto_impl(&, Box)]
#[automock]
pub trait BcStateProvider<S: DatabaseRef> {
    fn bc_state_at(
        &self,
        pos: TxPosition,
    ) -> Result<MemoryBcState<S>, SoflError>;
}
