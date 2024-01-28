use libsofl_core::engine::types::{
    BlockEnv, Bytecode, BytecodeState, CfgEnv, JumpMap, TxEnv, B256, U256,
};

use super::ConvertTo;

impl ConvertTo<U256> for reth_primitives::U256 {
    fn cvt(self) -> U256 {
        let be: [u8; 32] = self.to_be_bytes();
        U256::from_be_bytes(be)
    }
}

impl ConvertTo<B256> for reth_primitives::B256 {
    fn cvt(self) -> B256 {
        B256::from(self.0)
    }
}

impl ConvertTo<Bytecode> for reth_primitives::Bytecode {
    fn cvt(self) -> Bytecode {
        self.0.cvt()
    }
}

impl ConvertTo<Bytecode> for reth_revm::primitives::Bytecode {
    fn cvt(self) -> Bytecode {
        let bc = self;
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

impl ConvertTo<CfgEnv> for reth_revm::primitives::CfgEnv {
    fn cvt(self) -> CfgEnv {
        let mut cfg = CfgEnv::default();
        cfg.chain_id = self.chain_id;
        cfg.kzg_settings = match self.kzg_settings {
            reth_revm::primitives::EnvKzgSettings::Default => {
                libsofl_core::engine::revm::primitives::EnvKzgSettings::Default
            }
            reth_revm::primitives::EnvKzgSettings::Custom(s) => {
                libsofl_core::engine::revm::primitives::EnvKzgSettings::Custom(
                    s,
                )
            }
        };
        cfg.perf_analyse_created_bytecodes = match self
            .perf_analyse_created_bytecodes
        {
            reth_revm::primitives::AnalysisKind::Raw => {
                libsofl_core::engine::revm::primitives::AnalysisKind::Raw
            }
            reth_revm::primitives::AnalysisKind::Check => {
                libsofl_core::engine::revm::primitives::AnalysisKind::Check
            }
            reth_revm::primitives::AnalysisKind::Analyse => {
                libsofl_core::engine::revm::primitives::AnalysisKind::Analyse
            }
        };
        cfg.limit_contract_code_size = self.limit_contract_code_size;
        cfg.memory_limit = 2u64.pow(32) - 1;
        cfg.disable_balance_check = false;
        cfg.disable_block_gas_limit = false;
        cfg.disable_eip3607 = false;
        cfg.disable_gas_refund = false;
        cfg.disable_base_fee = false;
        cfg.disable_beneficiary_reward = false;
        cfg
    }
}

impl ConvertTo<reth_revm::primitives::CfgEnv> for CfgEnv {
    fn cvt(self) -> reth_revm::primitives::CfgEnv {
        let mut cfg = reth_revm::primitives::CfgEnv::default();
        cfg.chain_id = self.chain_id;
        cfg.kzg_settings = match self.kzg_settings {
            libsofl_core::engine::revm::primitives::EnvKzgSettings::Default => {
                reth_revm::primitives::EnvKzgSettings::Default
            }
            libsofl_core::engine::revm::primitives::EnvKzgSettings::Custom(
                s,
            ) => reth_revm::primitives::EnvKzgSettings::Custom(s),
        };
        cfg.perf_analyse_created_bytecodes = match self
            .perf_analyse_created_bytecodes
        {
            libsofl_core::engine::revm::primitives::AnalysisKind::Raw => {
                reth_revm::primitives::AnalysisKind::Raw
            }
            libsofl_core::engine::revm::primitives::AnalysisKind::Check => {
                reth_revm::primitives::AnalysisKind::Check
            }
            libsofl_core::engine::revm::primitives::AnalysisKind::Analyse => {
                reth_revm::primitives::AnalysisKind::Analyse
            }
        };
        cfg.limit_contract_code_size = self.limit_contract_code_size;
        cfg
    }
}

impl ConvertTo<BlockEnv> for reth_revm::primitives::BlockEnv {
    fn cvt(self) -> BlockEnv {
        let mut block = BlockEnv::default();
        block.number = self.number.cvt();
        block.coinbase = self.coinbase.cvt();
        block.timestamp = self.timestamp.cvt();
        block.gas_limit = self.gas_limit.cvt();
        block.basefee = self.basefee.cvt();
        block.difficulty = self.difficulty.cvt();
        block.prevrandao = self.prevrandao.map(|p| p.cvt());
        block.blob_excess_gas_and_price =
            self.blob_excess_gas_and_price.map(|b| {
                libsofl_core::engine::revm::primitives::BlobExcessGasAndPrice {
                    excess_blob_gas: b.excess_blob_gas,
                    blob_gasprice: b.blob_gasprice,
                }
            });
        block
    }
}

impl ConvertTo<reth_revm::primitives::BlockEnv> for BlockEnv {
    fn cvt(self) -> reth_revm::primitives::BlockEnv {
        let mut block = reth_revm::primitives::BlockEnv::default();
        block.number = self.number.cvt();
        block.coinbase = self.coinbase.cvt();
        block.timestamp = self.timestamp.cvt();
        block.gas_limit = self.gas_limit.cvt();
        block.basefee = self.basefee.cvt();
        block.difficulty = self.difficulty.cvt();
        block.prevrandao = self.prevrandao.map(|p| p.cvt());
        block.blob_excess_gas_and_price =
            self.blob_excess_gas_and_price.map(|b| {
                reth_revm::primitives::BlobExcessGasAndPrice {
                    excess_blob_gas: b.excess_blob_gas,
                    blob_gasprice: b.blob_gasprice,
                }
            });
        block
    }
}

impl ConvertTo<TxEnv> for reth_revm::primitives::TxEnv {
    fn cvt(self) -> TxEnv {
        let mut tx = TxEnv::default();
        tx.caller = self.caller.cvt();
        tx.gas_limit = self.gas_limit;
        tx.gas_price = self.gas_price.cvt();
        tx.transact_to = match self.transact_to {
            reth_revm::primitives::TransactTo::Create(scheme) => {
                let scheme = match scheme {
                    reth_revm::primitives::CreateScheme::Create => {
                        libsofl_core::engine::revm::primitives::CreateScheme::Create
                    }
                    reth_revm::primitives::CreateScheme::Create2 { salt } => {
                        libsofl_core::engine::revm::primitives::CreateScheme::Create2 {
                            salt: salt.cvt(),
                        }
                    }
                };
                libsofl_core::engine::revm::primitives::TransactTo::Create(
                    scheme,
                )
            }
            reth_revm::primitives::TransactTo::Call(addr) => {
                libsofl_core::engine::revm::primitives::TransactTo::Call(
                    addr.cvt(),
                )
            }
        };
        tx.value = self.value.cvt();
        tx.data = self.data.cvt();
        tx.nonce = self.nonce;
        tx.chain_id = self.chain_id;
        tx.access_list = self
            .access_list
            .into_iter()
            .map(|(addr, slots)| {
                (
                    addr.cvt(),
                    slots
                        .into_iter()
                        .map(|slot| slot.cvt())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        tx.gas_priority_fee = self.gas_priority_fee.map(ConvertTo::<U256>::cvt);
        tx.blob_hashes = self
            .blob_hashes
            .into_iter()
            .map(ConvertTo::<B256>::cvt)
            .collect();
        tx.max_fee_per_blob_gas =
            self.max_fee_per_blob_gas.map(ConvertTo::<U256>::cvt);
        tx
    }
}

impl ConvertTo<reth_revm::primitives::TxEnv> for TxEnv {
    fn cvt(self) -> reth_revm::primitives::TxEnv {
        let mut tx = reth_revm::primitives::TxEnv::default();
        tx.caller = self.caller.cvt();
        tx.gas_limit = self.gas_limit;
        tx.gas_price = self.gas_price.cvt();
        tx.transact_to = match self.transact_to {
            libsofl_core::engine::revm::primitives::TransactTo::Create(
                scheme,
            ) => {
                let scheme = match scheme {
                    libsofl_core::engine::revm::primitives::CreateScheme::Create => {
                        reth_revm::primitives::CreateScheme::Create
                    }
                    libsofl_core::engine::revm::primitives::CreateScheme::Create2 {
                        salt,
                    } => reth_revm::primitives::CreateScheme::Create2 { salt },
                };
                reth_revm::primitives::TransactTo::Create(scheme)
            }
            libsofl_core::engine::revm::primitives::TransactTo::Call(addr) => {
                reth_revm::primitives::TransactTo::Call(addr.cvt())
            }
        };
        tx.value = self.value.cvt();
        tx.data = self.data.cvt();
        tx.nonce = self.nonce;
        tx.chain_id = self.chain_id;
        tx.access_list = self
            .access_list
            .into_iter()
            .map(|(addr, slots)| {
                (
                    addr.cvt(),
                    slots
                        .into_iter()
                        .map(|slot| slot.cvt())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        tx.gas_priority_fee = self.gas_priority_fee.map(ConvertTo::<U256>::cvt);
        tx.blob_hashes = self
            .blob_hashes
            .into_iter()
            .map(ConvertTo::<B256>::cvt)
            .collect();
        tx.max_fee_per_blob_gas =
            self.max_fee_per_blob_gas.map(ConvertTo::<U256>::cvt);
        tx
    }
}
