use mockall::automock;

use crate::{
    engine::types::{Address, Bytes, TxHash, U256, Hash, TxEnv},
    error::SoflError,
};

use super::tx_position::TxPosition;

#[automock]
pub trait Tx {
    /// Returns the hash of the transaction.
    fn hash(&self) -> TxHash;

    /// Returns the sender of the transaction.
    fn sender(&self) -> Address;

    /// Returns the value of the transaction.
    fn value(&self) -> U256;

    /// Returns the input data of the transaction.
    fn input(&self) -> Bytes;

    /// Fill the revm transaction environment.
    fn fill_tx_env(&self, env: &mut TxEnv) -> Result<(), SoflError>;

    ////////////////
    /// Only available after tx execution
    ////////////////

    /// Returns the position of the transaction in the blockchain.
    /// None if the transaction is not in the blockchain.
    fn position(&self) -> Option<TxPosition>;

    /// Returns the output data of the transaction.
    /// None if the transaction is not executed.
    fn output(&self) -> Option<Bytes>;

    /// Returns whether the transaction succeeds.
    /// None if the transaction is not executed.
    fn success(&self) -> Option<bool>;

    /// Returns the gas used by the transaction.
    /// None if the transaction is not executed.
    fn logs(&self) -> Option<Vec<Log>>;

}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<Hash>,
    pub data: Bytes,
}