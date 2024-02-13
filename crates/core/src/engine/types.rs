use crate::blockchain::tx_position::TxPosition;

/// This module defines a set of types that are used throughout the library.
/// Most of types are re-exported from revm.

// Low level types
pub type Uint<const BITS: usize, const LIMBS: usize> =
    alloy_primitives::ruint::Uint<BITS, LIMBS>;
pub type Signed<const BITS: usize, const LIMBS: usize> =
    alloy_primitives::Signed<BITS, LIMBS>;
pub type U64 = alloy_primitives::ruint::Uint<64, 1>;
pub type U128 = alloy_primitives::ruint::Uint<128, 2>;
pub type U256 = alloy_primitives::U256;
pub type I256 = alloy_primitives::I256;
pub type B256 = alloy_primitives::B256;

// High level types
pub type Address = alloy_primitives::Address;
pub type Hash = alloy_primitives::B256;
pub type Bytes = alloy_primitives::Bytes;
pub type Bytecode = revm::primitives::Bytecode;
pub type BytecodeState = revm::primitives::BytecodeState;
pub type JumpMap = revm::primitives::JumpMap;

#[derive(Clone, Debug, derive_more::Deref)]
#[deref(forward)]
pub struct Hex(String);

pub type BlockNumber = alloy_primitives::BlockNumber;
pub type BlockHash = alloy_primitives::BlockHash;
#[derive(
    Clone,
    Debug,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Display,
)]
pub enum BlockHashOrNumber {
    #[display(fmt = "number({})", _0)]
    Hash(BlockHash),
    #[display(fmt = "hash({})", _0)]
    Number(BlockNumber),
}

pub type TxHash = alloy_primitives::TxHash;
pub type TxIndex = alloy_primitives::TxIndex;
#[derive(
    Clone,
    Debug,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Display,
)]
pub enum TxHashOrPosition {
    #[display(fmt = "hash({})", _0)]
    Hash(TxHash),
    #[display(fmt = "position({})", _0)]
    Position(TxPosition),
}

pub type ChainId = alloy_primitives::ChainId;
pub type SpecId = revm::primitives::SpecId;

pub type FixedBytes<const M: usize> = revm::primitives::FixedBytes<M>;
pub type AccountInfo = revm::primitives::AccountInfo;
pub type Account = revm::primitives::Account;
pub type AccountStatus = revm::primitives::AccountStatus;
pub type StorageKey = alloy_primitives::StorageKey;
pub type StorageValue = alloy_primitives::StorageValue;
pub type Storage =
    revm::primitives::HashMap<U256, revm::primitives::StorageSlot>;
pub type StorageSlot = revm::primitives::StorageSlot;

pub use revm::db::DatabaseRef;
pub use revm::handler::Handler;
pub use revm::interpreter::opcode;
pub use revm::interpreter::Host;
pub use revm::precompile::PrecompileSpecId;
pub use revm::precompile::Precompiles;
pub use revm::primitives::HandlerCfg;
pub use revm::Database;
pub use revm::DatabaseCommit;
pub use revm::Evm;
pub use revm::EvmBuilder;
pub use revm::JournaledState;
pub trait BcStateRef: revm::db::DatabaseRef + Sync + Send {}
impl<T: revm::db::DatabaseRef + Sync + Send> BcStateRef for T {}
pub use revm::Inspector;
pub type InstructionResult = revm::interpreter::InstructionResult;
pub type Interpreter = revm::interpreter::Interpreter;
pub type EvmContext<D> = revm::EvmContext<D>;
pub type Env = revm::primitives::Env;
pub type AnalysisKind = revm::primitives::AnalysisKind;
pub type BlobExcessGasAndPrice = revm::primitives::BlobExcessGasAndPrice;

pub type Context<EXT, DB> = revm::Context<EXT, DB>;

pub type TransactTo = revm::primitives::TransactTo;
pub type Transfer = revm::interpreter::Transfer;
pub type TxEnv = revm::primitives::TxEnv;
pub type BlockEnv = revm::primitives::BlockEnv;
pub type CfgEnv = revm::primitives::CfgEnv;
pub type StateChange = revm::primitives::State;
pub type SharedMemory = revm::interpreter::SharedMemory;
pub type Stack = revm::interpreter::Stack;
pub type JournalCheckpoint = revm::JournalCheckpoint;
pub type ExecutionResult = revm::primitives::ExecutionResult;
pub type Gas = revm::interpreter::Gas;
pub type InterpreterResult = revm::interpreter::InterpreterResult;
pub type InterpreterAction = revm::interpreter::InterpreterAction;
pub type BytecodeLocked = revm::interpreter::BytecodeLocked;
pub type Contract = revm::interpreter::Contract;
pub type CallFrame = revm::CallFrame;
pub type FrameData = revm::FrameData;
pub type FrameResult = revm::FrameResult;
pub type CreateFrame = revm::CreateFrame;
pub type Frame = revm::Frame;
pub type CreateInputs = revm::interpreter::CreateInputs;
pub type CallInputs = revm::interpreter::CallInputs;
pub type CallOutcome = revm::interpreter::CallOutcome;
pub type CallContext = revm::interpreter::CallContext;
pub type CallScheme = revm::interpreter::CallScheme;
pub type Output = revm::primitives::Output;
pub type CreateScheme = revm::primitives::CreateScheme;
pub type CreateOutcome = revm::interpreter::CreateOutcome;

pub const KECCAK_EMPTY: B256 = revm::primitives::KECCAK_EMPTY;
pub use revm::primitives::keccak256;

#[cfg(test)]
mod tests {
    #[test]
    fn test_type_alias() {
        use super::*;
        let x = U256::from(0);
        let y = revm::primitives::U256::from(1);
        let z = x == y;
        assert_eq!(z, false);
    }
}
