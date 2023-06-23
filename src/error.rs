use crate::engine::transaction::TxPosition;

#[derive(Debug)]
pub enum SoflError<DBERR = reth_interfaces::Error> {
    /// Wrapper of reth error
    Reth(reth_interfaces::Error),

    /// Fork position not found
    Fork(TxPosition),

    /// Wrapper of EVM error
    Evm(revm_primitives::EVMError<DBERR>),
}

impl<DBERR> From<reth_interfaces::Error> for SoflError<DBERR> {
    fn from(e: reth_interfaces::Error) -> Self {
        Self::Reth(e)
    }
}
