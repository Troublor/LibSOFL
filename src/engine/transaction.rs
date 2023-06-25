use derive_more::From;
use std::{
    error::Error,
    fmt::Display,
    ops::{
        Add, AddAssign, Deref, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
    },
};

use reth_primitives::{BlockHashOrNumber, TxHash};

use reth_provider::{BlockNumProvider, TransactionsProvider};
use revm::Database;
use revm_primitives::{Address, B256};
use serde::{Deserialize, Serialize};

pub type StateChange = revm_primitives::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxPositionOutOfRangeError {
    /// Block number is not in the range of the blockchain
    BlockOverflow((u64, u64)), // (max, requested)
    /// Transaction index is not in the range of the block
    IndexOverflow((u64, u64)), // (max, requested)
    /// The block hash is not known
    UnknownHash(B256),
}

impl TxPositionOutOfRangeError {
    pub fn unknown_block(pos: TxPosition, p: &impl BlockNumProvider) -> Self {
        match pos.block {
            BlockHashOrNumber::Hash(hash) => Self::UnknownHash(hash),
            BlockHashOrNumber::Number(block) => {
                Self::BlockOverflow((p.last_block_number().unwrap(), block))
            }
        }
    }
}

impl Display for TxPositionOutOfRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxPositionOutOfRangeError::BlockOverflow((max, requested)) => write!(
                f,
                "Block number {} is not in the range of the blockchain (max: {})",
                requested, max
            ),
            TxPositionOutOfRangeError::IndexOverflow((max, requested)) => write!(
                f,
                "Transaction index {} is not in the range of the block (max: {})",
                requested, max
            ),
            TxPositionOutOfRangeError::UnknownHash(hash) => {
                write!(f, "Block hash {} is unknown", hash)
            }
        }
    }
}

impl Error for TxPositionOutOfRangeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxPosition {
    pub block: BlockHashOrNumber,
    pub index: u64,
}

impl TxPosition {
    pub fn new(block: u64, index: u64) -> Self {
        Self {
            block: BlockHashOrNumber::from(block),
            index,
        }
    }

    pub fn from(block: B256, index: u64) -> Self {
        Self {
            block: BlockHashOrNumber::from(block),
            index,
        }
    }
}

impl Display for TxPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.block {
            BlockHashOrNumber::Hash(hash) => {
                write!(f, "{}-{}", hash, self.index)
            }
            BlockHashOrNumber::Number(block) => {
                write!(f, "{}-{}", block, self.index)
            }
        }
    }
}

impl Add<u64> for TxPosition {
    type Output = Self;

    /// Shift the transaction index by `rhs`.
    fn add(self, rhs: u64) -> Self::Output {
        Self {
            block: self.block,
            index: self.index + rhs,
        }
    }
}
impl Shr<u64> for TxPosition {
    type Output = Self;

    /// Shift the block number by `rhs` and set the index to 0.
    /// If the block is a hash, this will panic.
    fn shr(self, rhs: u64) -> Self::Output {
        let BlockHashOrNumber::Number(n) = self.block else { panic!("TxPosition::shr: block is a hash (not a number)")};
        Self {
            block: BlockHashOrNumber::from(n.add(rhs)),
            index: 0,
        }
    }
}

impl ShrAssign<u64> for TxPosition {
    /// Shift the block number in place by `rhs` and set the index to 0.
    /// If the block is a hash, this will panic.
    fn shr_assign(&mut self, rhs: u64) {
        let BlockHashOrNumber::Number(n) = self.block else { panic!("TxPosition::shr_assign: block is a hash (not a number)")};
        self.block = BlockHashOrNumber::from(n.add(rhs));
        self.index = 0;
    }
}

impl Sub<u64> for TxPosition {
    type Output = Self;

    /// Shift the transaction index by `rhs`.
    /// If the index is less than `rhs`, this will panic.
    fn sub(self, rhs: u64) -> Self::Output {
        if self.index < rhs {
            panic!("TxPosition::sub: index underflow");
        }
        Self {
            block: self.block,
            index: self.index - rhs,
        }
    }
}

impl Shl<u64> for TxPosition {
    type Output = Self;

    /// Shift the block number by `rhs` and set the index to 0.
    /// If the block is a hash, this will panic.
    fn shl(self, rhs: u64) -> Self::Output {
        let BlockHashOrNumber::Number(n) = self.block else { panic!("TxPosition::shl: block is a hash (not a number)")};
        if n < rhs {
            panic!("TxPosition::shl: block number underflow");
        }
        Self {
            block: BlockHashOrNumber::from(n - rhs),
            index: 0,
        }
    }
}

impl ShlAssign<u64> for TxPosition {
    /// Shift the block number in place by `rhs` and set the index to 0.
    /// If the block is a hash, this will panic.
    fn shl_assign(&mut self, rhs: u64) {
        let BlockHashOrNumber::Number(n) = self.block else { panic!("TxPosition::shl_assign: block is a hash (not a number)")};
        if n < rhs {
            panic!("TxPosition::shl_assign: block number underflow");
        }
        self.block = BlockHashOrNumber::from(n - rhs);
        self.index = 0;
    }
}

impl AddAssign<u64> for TxPosition {
    /// Shift the transaction index in place by `rhs`.
    fn add_assign(&mut self, rhs: u64) {
        self.index += rhs;
    }
}

impl SubAssign<u64> for TxPosition {
    /// Shift the transaction index in place by `rhs`.
    /// If the index is less than `rhs`, this will panic.
    fn sub_assign(&mut self, rhs: u64) {
        if self.index < rhs {
            panic!("TxPosition::sub_assign: index underflow");
        }
        self.index -= rhs;
    }
}

impl TxPosition {
    pub fn get_block_number(
        &self,
        p: &impl TransactionsProvider,
    ) -> Result<u64, TxPositionOutOfRangeError> {
        match self.block {
            BlockHashOrNumber::Hash(h) => {
                if let Ok(bn) = p.block_number(h) {
                    bn.ok_or(TxPositionOutOfRangeError::UnknownHash(h))
                } else {
                    Err(TxPositionOutOfRangeError::UnknownHash(h))
                }
            }
            BlockHashOrNumber::Number(n) => Ok(n),
        }
    }

    pub fn get_block_hash(
        &self,
        p: &impl TransactionsProvider,
    ) -> Result<B256, TxPositionOutOfRangeError> {
        match self.block {
            BlockHashOrNumber::Hash(h) => Ok(h),
            BlockHashOrNumber::Number(n) => {
                if let Ok(bh) = p.block_hash(n) {
                    bh.ok_or(TxPositionOutOfRangeError::BlockOverflow((
                        p.last_block_number().unwrap(),
                        n,
                    )))
                } else {
                    Err(TxPositionOutOfRangeError::BlockOverflow((
                        p.last_block_number().unwrap(),
                        n,
                    )))
                }
            }
        }
    }

    // shift the transaction position in history provided by TransactionsProvider by `offset`
    pub fn shift(
        &mut self,
        p: &impl TransactionsProvider,
        offset: i64,
    ) -> Result<(), TxPositionOutOfRangeError> {
        let get_txs_count = |block: BlockHashOrNumber| -> Result<u64, TxPositionOutOfRangeError> {
            if let Ok(txs) = p.transactions_by_block(block) {
                txs.map(|txs| txs.len() as u64)
                .ok_or(match block {
                    BlockHashOrNumber::Hash(h) => TxPositionOutOfRangeError::UnknownHash(h),
                    BlockHashOrNumber::Number(n) => TxPositionOutOfRangeError::BlockOverflow((
                        p.last_block_number().unwrap(),
                        n,
                    )),
                })
            } else {
                match block {
                    BlockHashOrNumber::Hash(h) => Err(TxPositionOutOfRangeError::UnknownHash(h)),
                    BlockHashOrNumber::Number(n) => Err(TxPositionOutOfRangeError::BlockOverflow((
                        p.last_block_number().unwrap(),
                        n,
                    ))),
                }
            }
        };
        if let BlockHashOrNumber::Hash(h) = self.block {
            self.block = self
                .get_block_number(p)
                .map(BlockHashOrNumber::from)
                .map_err(|_| TxPositionOutOfRangeError::UnknownHash(h))?;
        }
        match offset {
            0 => Ok(()),
            1_i64..=i64::MAX => {
                let mut cur_txs_count = get_txs_count(self.block)?;
                let mut offset = offset as u64;
                while self.index + offset >= cur_txs_count {
                    offset -= cur_txs_count - self.index;
                    self.shr_assign(1);
                    cur_txs_count = get_txs_count(self.block)?;
                }
                self.add_assign(offset);
                Ok(())
            }
            i64::MIN..=-1_i64 => {
                let mut offset = offset.unsigned_abs();
                while self.index < offset {
                    offset -= self.index + 1;
                    self.shl_assign(1);
                    self.index = get_txs_count(self.block)? - 1;
                }
                self.sub_assign(offset);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::path::Path;

    use crate::{
        config::flags::SoflConfig, engine::providers::BcProviderBuilder,
    };

    use super::TxPosition;

    #[test]
    fn test_shift() {
        let cfg = SoflConfig::load().unwrap();
        let datadir = Path::new(cfg.reth.datadir.as_str());
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

        let mut pos = TxPosition::new(16000000, 0);
        pos.shift(&bp, -1).unwrap();
        assert_eq!(pos, TxPosition::new(15999999, 260));

        let mut pos = TxPosition::new(16000000, 210);
        pos.shift(&bp, 1).unwrap();
        assert_eq!(pos, TxPosition::new(16000001, 0));

        let mut pos = TxPosition::new(16000000, 10);
        pos.shift(&bp, -10).unwrap();
        assert_eq!(pos, TxPosition::new(16000000, 0));

        let mut pos = TxPosition::new(16000000, 10);
        pos.shift(&bp, -20).unwrap();
        assert_eq!(pos, TxPosition::new(15999999, 251));

        let mut pos = TxPosition::new(16000000, 10);
        pos.shift(&bp, 100).unwrap();
        assert_eq!(pos, TxPosition::new(16000000, 110));

        let mut pos = TxPosition::new(16000000, 10);
        pos.shift(&bp, 1000).unwrap();
        assert_eq!(pos, TxPosition::new(16000008, 59));
    }
}

#[derive(From)]
pub enum TxOrPseudo<'a, S> {
    Tx(Tx),
    Pseudo(&'a dyn Fn(&S) -> StateChange),
}

impl<S> TxOrPseudo<'_, S> {
    pub fn is_pseudo(&self) -> bool {
        matches!(self, TxOrPseudo::Pseudo(_))
    }
}

impl<'a, S> Clone for TxOrPseudo<'a, S> {
    fn clone(&self) -> Self {
        match self {
            TxOrPseudo::Tx(tx) => TxOrPseudo::Tx(tx.clone()),
            TxOrPseudo::Pseudo(_) => panic!("cannot clone pseudo tx"),
        }
    }
}

impl<'a, S: Database> Deref for TxOrPseudo<'a, S> {
    type Target = Tx;

    fn deref(&self) -> &Self::Target {
        match self {
            TxOrPseudo::Tx(tx) => tx,
            TxOrPseudo::Pseudo(_) => panic!("cannot deref pseudo tx"),
        }
    }
}

impl<'a, S: Database> AsRef<Tx> for TxOrPseudo<'a, S> {
    fn as_ref(&self) -> &Tx {
        match self {
            TxOrPseudo::Tx(tx) => tx,
            TxOrPseudo::Pseudo(_) => panic!("cannot deref pseudo tx"),
        }
    }
}

impl<'a, S: Database> From<reth_primitives::TransactionSigned>
    for TxOrPseudo<'a, S>
{
    fn from(tx: reth_primitives::TransactionSigned) -> Self {
        Tx::Signed(tx).into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, From)]
pub enum Tx {
    Signed(reth_primitives::TransactionSigned),
    Unsigned((Address, reth_primitives::Transaction)),
}

impl Deref for Tx {
    type Target = reth_primitives::Transaction;

    fn deref(&self) -> &Self::Target {
        match self {
            Tx::Signed(tx) => tx,
            Tx::Unsigned((_, tx)) => tx,
        }
    }
}

impl AsRef<reth_primitives::Transaction> for Tx {
    fn as_ref(&self) -> &reth_primitives::Transaction {
        match self {
            Tx::Signed(tx) => tx,
            Tx::Unsigned((_, tx)) => tx,
        }
    }
}

impl Tx {
    pub fn from(&self) -> Address {
        match self {
            Tx::Signed(tx) => tx.recover_signer().unwrap(),
            Tx::Unsigned((sender, _)) => *sender,
        }
    }

    pub fn to(&self) -> Option<Address> {
        match self {
            Tx::Signed(tx) => tx.to(),
            Tx::Unsigned((_, tx)) => tx.to(),
        }
    }

    pub fn hash(&self) -> TxHash {
        match self {
            Tx::Signed(tx) => tx.hash(),
            _ => TxHash::zero(),
        }
    }
}
