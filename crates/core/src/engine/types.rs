use crate::blockchain::tx_position::TxPosition;

/// This module defines a set of types that are used throughout the library.
/// Most of types are re-exported from revm.

// Low level types
pub type Uint<const BITS: usize, const LIMBS: usize> = revm::primitives::ruint::Uint<BITS, LIMBS>;
pub type U64 = revm::primitives::ruint::Uint<64, 1>;
pub type U128 = revm::primitives::ruint::Uint<128, 2>;
pub type U256 = revm::primitives::U256;
pub type I256 = revm::primitives::I256;
pub type B256 = revm::primitives::B256;

// High level types
pub type Address = revm::primitives::Address;
pub type Hash = revm::primitives::B256;
pub type Bytes = revm::primitives::Bytes;
pub type Bytecode = revm::primitives::Bytecode;

#[derive(Clone, Debug, derive_more::Deref)]
#[deref(forward)]
pub struct Hex (String);

pub type BlockNumber = alloy_primitives::BlockNumber;
pub type BlockHash = alloy_primitives::BlockHash;
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, derive_more::From, derive_more::Display)]
pub enum BlockHashOrNumber {
    #[display(fmt = "number({})", _0)]
    Hash(BlockHash),
    #[display(fmt = "hash({})", _0)]
    Number(BlockNumber),
}

pub type TxHash = alloy_primitives::TxHash;
pub type TxIndex = alloy_primitives::TxIndex;
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, derive_more::From, derive_more::Display)]
pub enum TxHashOrPosition {
    #[display(fmt = "hash({})", _0)]
    Hash(TxHash),
    #[display(fmt = "position({})", _0)]
    Position(TxPosition),
}

pub type ChainId = alloy_primitives::ChainId;
pub type SpecId = revm::primitives::SpecId;

pub type AccountInfo = revm::primitives::AccountInfo;
pub type Account = revm::primitives::Account;
pub type AccountStatus = revm::primitives::AccountStatus;
pub type StorageKey = alloy_primitives::StorageKey;
pub type StorageValue = alloy_primitives::StorageValue;
pub type Storage = revm::primitives::HashMap<U256, revm::primitives::StorageSlot>;
pub type StorageSlot = revm::primitives::StorageSlot;

pub use revm::Inspector;
pub use revm::interpreter::opcode;
pub type InstructionResult = revm::interpreter::InstructionResult;
pub type Interpreter<'a> = revm::interpreter::Interpreter<'a>;
pub type EVMData<'a, D> = revm::EVMData<'a, D>;
pub type AnalysisKind = revm::primitives::AnalysisKind;
pub type BlobExcessGasAndPrice = revm::primitives::BlobExcessGasAndPrice;

pub type TransactTo = revm::primitives::TransactTo;
pub type TxEnv = revm::primitives::TxEnv;
pub type BlockEnv = revm::primitives::BlockEnv;
pub type CfgEnv = revm::primitives::CfgEnv;
pub type StateChange = revm::primitives::State;
pub type ExecutionResult = revm::primitives::ExecutionResult;
pub type Output = revm::primitives::Output;
pub type CreateScheme = revm::primitives::CreateScheme;


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