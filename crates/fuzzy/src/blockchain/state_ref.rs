use std::{path::Path, sync::Arc};

use libsofl_core::{
    blockchain::provider::BcProvider,
    engine::types::{
        AccountInfo, Address, BcStateRef, Bytecode, ChainId, B256, U256,
    },
    error::SoflError,
};
use libsofl_reth::blockchain::{
    provider::{RethProvider, StateProviderFactory},
    state::{ProviderError, RethBcStateRef},
};
use revm::{db::EmptyDB, DatabaseRef};
use serde::{
    de::{self, Visitor},
    ser::SerializeStruct,
};

pub trait FuzzBcStateRef:
    BcStateRef
    + Clone
    + std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
    fn chain_id(&self) -> ChainId;
    fn block_number(&self) -> u64;
}

// ============== EMPTY DB ==============

impl FuzzBcStateRef for EmptyDB {
    fn chain_id(&self) -> ChainId {
        ChainId::default()
    }

    fn block_number(&self) -> u64 {
        0
    }
}

// ============== Reth DB ===============

#[derive(derive_more::AsRef, derive_more::Deref, derive_more::DerefMut)]
pub struct SerializableRethStateRef {
    pub datadir: String,
    pub chain_id: ChainId,
    pub block_number: u64,

    #[as_ref]
    #[deref]
    #[deref_mut]
    pub state_ref: Arc<RethBcStateRef>,
}

impl SerializableRethStateRef {
    pub fn new(
        datadir: String,
        chain_id: ChainId,
        block_number: u64,
    ) -> Result<Self, SoflError> {
        let provider = RethProvider::from_db(Path::new(datadir.as_str()))?;
        assert_eq!(provider.chain_id(), chain_id);
        let state_ref = provider
            .state_by_block_id(block_number.into())
            .map_err(|e| SoflError::Provider(format!("{:?}", e)))?;
        Ok(Self {
            datadir,
            chain_id,
            block_number,
            state_ref: Arc::new(state_ref.into()),
        })
    }
}

impl FuzzBcStateRef for SerializableRethStateRef {
    fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    fn block_number(&self) -> u64 {
        self.block_number
    }
}

impl std::fmt::Debug for SerializableRethStateRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerializableRethStateRef")
            .field("datadir", &self.datadir)
            .field("chain_id", &self.chain_id)
            .field("block_number", &self.block_number)
            .finish()
    }
}

impl Clone for SerializableRethStateRef {
    fn clone(&self) -> Self {
        Self {
            datadir: self.datadir.clone(),
            chain_id: self.chain_id,
            block_number: self.block_number,
            state_ref: self.state_ref.clone(),
        }
    }
}

impl serde::Serialize for SerializableRethStateRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state =
            serializer.serialize_struct("SerializableRethStateRef", 3)?;
        state.serialize_field("datadir", &self.datadir)?;
        state.serialize_field("chain_id", &self.chain_id)?;
        state.serialize_field("block_number", &self.block_number)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for SerializableRethStateRef {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct SerializableRethStateRefData {
            datadir: String,
            chain_id: ChainId,
            block_number: u64,
        }

        struct DataVisitor;

        impl<'de> Visitor<'de> for DataVisitor {
            type Value = SerializableRethStateRefData;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("struct SerializableRethStateRefData")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let datadir = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let chain_id = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let block_number = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                Ok(SerializableRethStateRefData {
                    datadir,
                    chain_id,
                    block_number,
                })
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut datadir = None;
                let mut chain_id = None;
                let mut block_number = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        "datadir" => {
                            if datadir.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "datadir",
                                ));
                            }
                            datadir = Some(map.next_value()?);
                        }
                        "chain_id" => {
                            if chain_id.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "chain_id",
                                ));
                            }
                            chain_id = Some(map.next_value()?);
                        }
                        "block_number" => {
                            if block_number.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "block_number",
                                ));
                            }
                            block_number = Some(map.next_value()?);
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                key,
                                &["datadir", "chain_id", "block_number"],
                            ));
                        }
                    }
                }
                let datadir = datadir.ok_or_else(|| {
                    serde::de::Error::missing_field("datadir")
                })?;
                let chain_id = chain_id.ok_or_else(|| {
                    serde::de::Error::missing_field("chain_id")
                })?;
                let block_number = block_number.ok_or_else(|| {
                    serde::de::Error::missing_field("block_number")
                })?;
                Ok(SerializableRethStateRefData {
                    datadir,
                    chain_id,
                    block_number,
                })
            }
        }

        let data = _deserializer.deserialize_struct(
            "SerializableRethStateRef",
            &["datadir", "chain_id", "block_number"],
            DataVisitor,
        )?;
        Ok(
            Self::new(data.datadir, data.chain_id, data.block_number).expect(
                "failed to deserialize (reconstruct) SerializableRethStateRef",
            ),
        )
    }
}

impl DatabaseRef for SerializableRethStateRef {
    type Error = ProviderError;

    fn basic_ref(
        &self,
        address: Address,
    ) -> Result<Option<AccountInfo>, Self::Error> {
        self.state_ref.basic_ref(address)
    }

    fn code_by_hash_ref(
        &self,
        code_hash: B256,
    ) -> Result<Bytecode, Self::Error> {
        self.state_ref.code_by_hash_ref(code_hash)
    }

    fn storage_ref(
        &self,
        address: Address,
        index: U256,
    ) -> Result<U256, Self::Error> {
        self.state_ref.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: U256) -> Result<B256, Self::Error> {
        self.state_ref.block_hash_ref(number)
    }
}

#[cfg(test)]
mod tests_with_db {
    use libsofl_reth::config::RethConfig;
    use libsofl_utils::config::Config;

    #[test]
    fn test_serialize_reth_bc_state() {
        let reth_config = RethConfig::must_load();
        let state_ref = super::SerializableRethStateRef::new(
            reth_config.datadir,
            1,
            14000000,
        )
        .unwrap();
        let serialized = serde_json::to_string(&state_ref).unwrap();
        println!("{:?}", serialized.clone());
        let deserialized: super::SerializableRethStateRef =
            serde_json::from_str(serialized.as_str()).unwrap();
        assert_eq!(deserialized.chain_id, 1);
        assert_eq!(deserialized.block_number, 14000000);
    }
}
