use std::ops::Deref;

use reth_primitives::BlockHashOrNumber;

use revm::Database;
use revm_primitives::{Address, B256};

pub type StateChange = revm_primitives::State;

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

pub enum Tx<'a, S> {
    Signed(reth_primitives::TransactionSigned),
    Unsigned((Address, reth_primitives::Transaction)),
    Pseudo(&'a dyn Fn(&S) -> StateChange),
}

impl<'a, S> Clone for Tx<'a, S> {
    fn clone(&self) -> Self {
        match self {
            Tx::Signed(tx) => Tx::Signed(tx.clone()),
            Tx::Unsigned((sender, tx)) => Tx::Unsigned((*sender, tx.clone())),
            Tx::Pseudo(tx) => panic!("cannot clone pseudo tx"),
        }
    }
}

impl<'a, S: Database> Tx<'a, S> {
    pub fn valid(&self) -> bool {
        match self {
            Tx::Signed(tx) => tx.recover_signer().is_some(),
            Tx::Unsigned(_) => true,
            Tx::Pseudo(_) => true,
        }
    }

    pub fn sender(&self) -> Address {
        match self {
            Tx::Signed(tx) => tx.recover_signer().unwrap(),
            Tx::Unsigned((sender, _)) => *sender,
            Tx::Pseudo(_) => Address::zero(),
        }
    }
}

impl<'a, S: Database> Deref for Tx<'a, S> {
    type Target = reth_primitives::Transaction;

    fn deref(&self) -> &Self::Target {
        match self {
            Tx::Signed(tx) => &tx.transaction,
            Tx::Unsigned((_, tx)) => tx,
            Tx::Pseudo(_) => panic!("cannot deref pseudo tx"),
        }
    }
}

impl<'a, S: Database> AsRef<reth_primitives::Transaction> for Tx<'a, S> {
    fn as_ref(&self) -> &reth_primitives::Transaction {
        match self {
            Tx::Signed(tx) => tx,
            Tx::Unsigned((_, tx)) => tx,
            Tx::Pseudo(_) => panic!("cannot deref pseudo tx"),
        }
    }
}
