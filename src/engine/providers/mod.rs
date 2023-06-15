use reth_provider::{EvmEnvProvider, StateProviderFactory, TransactionsProvider};

pub mod reth;
pub mod rpc;

pub trait BcProvider: EvmEnvProvider + TransactionsProvider + StateProviderFactory {}

pub struct BcProviderBuilder {
    // constructors are implemented in specific provider moduels
}
