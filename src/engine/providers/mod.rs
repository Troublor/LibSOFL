use reth_provider::{
    BlockProvider, EvmEnvProvider, StateProviderFactory, TransactionsProvider,
};

pub mod reth;
pub mod rpc;

pub trait BcProvider:
    EvmEnvProvider
    + TransactionsProvider
    + BlockProvider
    + StateProviderFactory
    + Send
    + Sync
    + Clone
{
}

pub struct BcProviderBuilder {
    // constructors are implemented in specific provider moduels
}
