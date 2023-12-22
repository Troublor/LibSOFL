use std::{
    fmt::Display,
    ops::{Add, AddAssign, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign},
};

use crate::{
    engine::types::{BlockHashOrNumber, B256, U256},
    error::SoflError,
};

use super::{provider::BcProvider, transaction::Tx};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl From<U256> for TxPosition {
    fn from(block: U256) -> Self {
        Self {
            block: BlockHashOrNumber::from(block.as_limbs()[3]),
            index: 0,
        }
    }
}

impl From<BlockHashOrNumber> for TxPosition {
    fn from(block: BlockHashOrNumber) -> Self {
        Self { block, index: 0 }
    }
}

impl From<u64> for TxPosition {
    fn from(block: u64) -> Self {
        Self {
            block: BlockHashOrNumber::from(block),
            index: 0,
        }
    }
}

impl From<(u64, u64)> for TxPosition {
    fn from((block, index): (u64, u64)) -> Self {
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
        let BlockHashOrNumber::Number(n) = self.block else {
                panic!("TxPosition::shr: block is a hash (not a number)")
            };
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
        let BlockHashOrNumber::Number(n) = self.block else {
                panic!("TxPosition::shr_assign: block is a hash (not a number)")
            };
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
        let BlockHashOrNumber::Number(n) = self.block else {
                panic!("TxPosition::shl: block is a hash (not a number)")
            };
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
        let BlockHashOrNumber::Number(n) = self.block else {
                panic!("TxPosition::shl_assign: block is a hash (not a number)")
            };
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
    /// Shift the transaction position in history provided by TxProvider by `offset`
    pub fn shift<T: Tx, P: BcProvider<T>>(
        &mut self,
        p: &P,
        offset: i64,
    ) -> Result<(), SoflError> {
        let get_txs_count = |block: BlockHashOrNumber| -> Result<u64, SoflError> {
            p.txs_in_block(block).map(|txs| txs.len() as u64)
        };
        if let BlockHashOrNumber::Hash(h) = self.block {
            self.block = p.block_number_by_hash(h).map(BlockHashOrNumber::from)?;
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
pub(crate) mod tests_with_db {
    use mockall::predicate::eq;

    use crate::{
        blockchain::{
            provider::MockBcProvider, transaction::MockTx,
        },
        engine::types::BlockHashOrNumber,
    };

    use super::TxPosition;

    #[test]
    fn test_shift() {
        let mut bp = MockBcProvider::<MockTx>::new();
        bp.expect_txs_in_block()
            .with(eq(BlockHashOrNumber::from(15999999)))
            .returning(|_| Ok((0..261).map(|_| MockTx::default()).collect()));
        bp.expect_txs_in_block()
            .with(eq(BlockHashOrNumber::from(16000000)))
            .returning(|_| Ok((0..211).map(|_| MockTx::default()).collect()));
        bp.expect_txs_in_block()
            .with(eq(BlockHashOrNumber::from(16000001)))
            .returning(|_| Ok((0..10).map(|_| MockTx::default()).collect()));
        bp.expect_txs_in_block()
            .with(eq(BlockHashOrNumber::from(16000002)))
            .returning(|_| Ok((0..100).map(|_| MockTx::default()).collect()));

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

        let mut pos = TxPosition::new(16000000, 0);
        pos.shift(&bp, 250).unwrap();
        assert_eq!(pos, TxPosition::new(16000002, 29));
    }
}
