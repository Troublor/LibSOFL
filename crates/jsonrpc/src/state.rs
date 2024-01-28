use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use alloy_providers::provider::TempProvider;
use alloy_rpc_types::BlockNumberOrTag;
use libsofl_core::{
    blockchain::{
        provider::{BcProvider, BcStateProvider},
        tx_position::TxPosition,
    },
    conversion::ConvertTo,
    engine::{
        memory::MemoryBcState,
        types::{
            keccak256, AccountInfo, Address, BlockHashOrNumber, Bytecode,
            DatabaseRef, Hash, B256, KECCAK_EMPTY, U256,
        },
    },
    error::SoflError,
};

use crate::provider::JsonRpcProvider;

pub struct JsonrRpcBcStateRef {
    pub(crate) provider: JsonRpcProvider,
    pub(crate) pos: TxPosition,
}

impl BcStateProvider<JsonrRpcBcStateRef> for JsonRpcProvider {
    /// Create a BcState from the the state before the transaction at the position is executed.
    fn bc_state_at(
        &self,
        pos: TxPosition,
    ) -> Result<MemoryBcState<JsonrRpcBcStateRef>, SoflError> {
        Ok(MemoryBcState::new(JsonrRpcBcStateRef {
            provider: self.clone(),
            pos,
        }))
    }
}

impl JsonrRpcBcStateRef {
    fn bn(&self) -> Result<u64, SoflError> {
        match self.pos.block {
            BlockHashOrNumber::Hash(hash) => {
                self.provider.block_number_by_hash(hash)
            }
            BlockHashOrNumber::Number(number) => Ok(number),
        }
    }
}

impl DatabaseRef for JsonrRpcBcStateRef {
    #[doc = " The database error type."]
    type Error = SoflError;

    #[doc = " Get basic account information."]
    fn basic_ref(
        &self,
        address: Address,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        let task = async {
            let bn = (self.bn()? - 1).into();
            let balance = self
                .provider
                .p
                .get_balance(address, Some(bn))
                .await
                .map_err(|e| {
                    SoflError::Provider(format!("failed to get balance: {}", e))
                })?;
            let nonce = self
                .provider
                .p
                .get_transaction_count(address, Some(bn))
                .await
                .map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to get transaction count: {}",
                        e
                    ))
                })?;
            let code: Bytecode = self
                .provider
                .p
                .get_code_at(address, bn)
                .await
                .map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to get code hash: {}",
                        e
                    ))
                })?
                .cvt();
            let code_hash = if code.is_empty() {
                KECCAK_EMPTY
            } else {
                keccak256(code.bytes())
            };
            get_code_hash_map()
                .lock()
                .unwrap()
                .entry(code_hash)
                .or_insert(code.clone());
            Ok(Some(AccountInfo {
                balance,
                nonce: nonce.cvt(),
                code_hash,
                code: Some(code),
            }))
        };
        self.provider.rt.block_on(task)
    }

    #[doc = " Get account code by its hash."]
    fn code_by_hash_ref(
        &self,
        code_hash: B256,
    ) -> Result<Bytecode, Self::Error> {
        get_code_hash_map()
            .lock()
            .unwrap()
            .get(&code_hash)
            .map(Clone::clone)
            .ok_or(SoflError::NotFound(format!("code hash {}", code_hash)))
    }

    #[doc = " Get storage value of address at index."]
    fn storage_ref(
        &self,
        address: Address,
        index: U256,
    ) -> Result<U256, Self::Error> {
        let task = async {
            let bn = (self.bn()? - 1).into();
            let value = self
                .provider
                .p
                .get_storage_at(address, index.cvt(), Some(bn))
                .await
                .map_err(|e| {
                    SoflError::Provider(format!("failed to get storage: {}", e))
                })?;
            Ok(value)
        };
        self.provider.rt.block_on(task)
    }

    #[doc = " Get block hash by block number."]
    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        let task = async {
            let blk = self
                .provider
                .p
                .get_block_by_number(
                    BlockNumberOrTag::Number(number.cvt()),
                    false,
                )
                .await
                .map_err(|e| {
                    SoflError::Provider(format!(
                        "failed to get block hash: {}",
                        e
                    ))
                })?
                .ok_or(SoflError::NotFound(format!(
                    "block number {}",
                    number
                )))?;
            blk.header
                .hash
                .ok_or(SoflError::NotFound(format!("block number {}", number)))
        };
        self.provider.rt.block_on(task)
    }
}

type CodeHashMap = Mutex<Option<Arc<Mutex<HashMap<Hash, Bytecode>>>>>;

/// Global map from code hash to code.
/// This global mapping is possible because we assume unique code hash must map to unique code.
static CODE_HASH_TO_CODE: CodeHashMap = Mutex::new(None);

fn get_code_hash_map() -> Arc<Mutex<HashMap<Hash, Bytecode>>> {
    let mut maybe_map = CODE_HASH_TO_CODE.lock().unwrap();
    if maybe_map.is_none() {
        let new_map = Arc::new(Mutex::new(HashMap::new()));
        *maybe_map = Some(new_map);
    }
    maybe_map.as_ref().unwrap().clone()
}
