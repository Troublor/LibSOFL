use std::io::Read;

use libsofl_core::engine::types::{
    Bytecode, BytecodeState, Bytes, JumpMap, B256, U256,
};

use super::ConvertTo;

impl ConvertTo<U256> for reth_primitives::U256 {
    fn cvt(self) -> U256 {
        let be = self.to_be_bytes();
        U256::from_be_bytes(be)
    }
}

impl ConvertTo<B256> for reth_primitives::B256 {
    fn cvt(self) -> B256 {
        let be = self.as_slice();
        B256::from_slice(be)
    }
}

impl ConvertTo<Bytecode> for reth_primitives::Bytecode {
    fn cvt(self) -> Bytecode {
        let bc = self.0;
        Bytecode {
            bytecode: bc.bytecode.cvt(),
            state: match bc.state {
                reth_revm::primitives::BytecodeState::Raw => BytecodeState::Raw,
                reth_revm::primitives::BytecodeState::Checked { len } => {
                    BytecodeState::Checked { len }
                }
                reth_revm::primitives::BytecodeState::Analysed {
                    len,
                    jump_map,
                    ..
                } => BytecodeState::Analysed {
                    len,
                    jump_map: JumpMap::from_slice(jump_map.as_slice()),
                },
            },
        }
    }
}
