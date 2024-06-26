use libsofl_core::{
    blockchain::transaction::Log,
    engine::types::{AccountInfo, Address, BlockHashOrNumber, Bytecode, Bytes},
};

use super::ConvertTo;

impl ConvertTo<Bytes> for reth_primitives::Bytes {
    fn cvt(self) -> Bytes {
        self.0.into()
    }
}

impl ConvertTo<reth_primitives::Address> for Address {
    fn cvt(self) -> Address {
        self.into()
    }
}

impl ConvertTo<reth_primitives::BlockHashOrNumber> for BlockHashOrNumber {
    fn cvt(self) -> reth_primitives::BlockHashOrNumber {
        match self {
            BlockHashOrNumber::Hash(hash) => {
                reth_primitives::BlockHashOrNumber::Hash(hash)
            }
            BlockHashOrNumber::Number(number) => {
                reth_primitives::BlockHashOrNumber::Number(number)
            }
        }
    }
}

impl ConvertTo<Log> for reth_primitives::Log {
    fn cvt(self) -> Log {
        Log {
            address: self.address.cvt(),
            topics: self.topics.into_iter().map(|h| h).collect(),
            data: self.data.cvt(),
        }
    }
}

impl ConvertTo<AccountInfo> for reth_revm::primitives::AccountInfo {
    fn cvt(self) -> AccountInfo {
        AccountInfo {
            balance: self.balance.cvt(),
            nonce: self.nonce,
            code_hash: self.code_hash.cvt(),
            code: self.code.map(|c| ConvertTo::<Bytecode>::cvt(c)),
        }
    }
}
