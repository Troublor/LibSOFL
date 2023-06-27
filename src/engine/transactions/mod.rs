pub mod position;
use derive_more::From;
use std::ops::Deref;

use reth_primitives::TxHash;

use revm::Database;
use revm_primitives::Address;
use serde::{Deserialize, Serialize};

pub type StateChange = revm_primitives::State;
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
