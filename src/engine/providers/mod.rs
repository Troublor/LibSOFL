use reth_provider::{
    BlockReader, EvmEnvProvider, StateProviderFactory, TransactionsProvider,
};

pub mod reth;
pub mod rpc;

pub trait BcProvider:
    EvmEnvProvider
    + TransactionsProvider
    + BlockReader
    + StateProviderFactory
    + Send
    + Sync
    + Clone
{
}

pub struct BcProviderBuilder {
    // constructors are implemented in specific provider moduels
}
