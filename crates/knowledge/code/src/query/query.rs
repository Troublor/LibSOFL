use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use alloy_json_abi::JsonAbi;
use foundry_block_explorers::{contract::Metadata, errors::EtherscanError};
use foundry_compilers::{
    artifacts::{
        output_selection::OutputSelection, Contract, Source, Sources,
        StorageLayout,
    }, Artifact, CompilerInput, CompilerOutput, Solc
};
use libsofl_core::engine::types::{Address, FixedBytes};
use libsofl_knowledge_base::config::KnowledgeConfig;
use moka::sync::Cache;
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait};
use semver::Version;

use crate::{config::CodeKnowledgeConfig, entities, error::Error};

pub struct CodeQuery {
    fetcher: super::fetcher::MultiplexedFetcher,
    eager: bool,
    db: DatabaseConnection,
    source_code_cache: Cache<Address, Arc<Metadata>>,
    compiler_input_cache: Cache<Address, (Version, Arc<CompilerInput>)>, // compiler version and input
    compiler_output_cache: Cache<Address, Arc<CompilerOutput>>,
    model_cache: Cache<Address, Arc<entities::code::Model>>,
    storage_layout_cache: Cache<Address, Arc<StorageLayout>>,
    abi_cache: Cache<Address, Arc<JsonAbi>>,
    function_signatures_cache:
        Cache<Address, Arc<BTreeMap<FixedBytes<4>, String>>>,
}

impl CodeQuery {
    pub async fn new(
        db_cfg: &KnowledgeConfig,
        cfg: &CodeKnowledgeConfig,
        eager: bool,
    ) -> Result<Self, Error> {
        let fetcher = super::fetcher::MultiplexedFetcher::new(cfg);
        let db = db_cfg
            .get_database_connection()
            .await
            .map_err(Error::Database)?;
        let source_code_cache = Cache::new(cfg.cache_size);
        let compiler_input_cache = Cache::new(cfg.cache_size);
        let compiler_output_cache = Cache::new(cfg.cache_size);
        Ok(Self {
            fetcher,
            db,
            eager,
            source_code_cache,
            compiler_input_cache,
            compiler_output_cache,
            model_cache: Cache::new(cfg.cache_size),
            storage_layout_cache: Cache::new(cfg.cache_size),
            abi_cache: Cache::new(cfg.cache_size),
            function_signatures_cache: Cache::new(cfg.cache_size),
        })
    }
}

impl CodeQuery {
    pub async fn get_abi_async(
        &self,
        address: Address,
    ) -> Result<Option<Arc<JsonAbi>>, Error> {
        // check cache first
        let abi = self.abi_cache.get(&address);
        if let Some(abi) = abi {
            return Ok(Some(abi));
        }

        let model = self.get_model_async(address).await?;
        if let Some(model) = model {
            let abi = model.abi();
            let abi = Arc::new(abi);
            self.abi_cache.insert(address, abi.clone());
            Ok(Some(abi))
        } else {
            Ok(None)
        }
    }

    pub async fn get_function_signatures_async(
        &self,
        address: Address,
    ) -> Result<Option<Arc<BTreeMap<FixedBytes<4>, String>>>, Error> {
        // check cache first
        let signatures = self.function_signatures_cache.get(&address);
        if let Some(signatures) = signatures {
            return Ok(Some(signatures));
        }

        let model = self.get_model_async(address).await?;
        if let Some(model) = model {
            let signatures = model.function_signatures();
            let signatures = Arc::new(signatures);
            self.function_signatures_cache
                .insert(address, signatures.clone());
            Ok(Some(signatures))
        } else {
            Ok(None)
        }
    }

    pub async fn get_storage_layout_async(
        &self,
        address: Address,
    ) -> Result<Option<Arc<StorageLayout>>, Error> {
        // check cache first
        let layout = self.storage_layout_cache.get(&address);
        if let Some(layout) = layout {
            return Ok(Some(layout));
        }

        let model = self.get_model_async(address).await?;
        if let Some(model) = model {
            if model.compiler_version().minor < 4 {
                Err(Error::SolidityVersionTooLow)
            } else {
                let layout = serde_json::from_value::<StorageLayout>(
                    model.storage_layout.clone(),
                )
                .expect("failed to deserialize storage layout");
                let layout = Arc::new(layout);
                self.storage_layout_cache.insert(address, layout.clone());
                Ok(Some(layout))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn get_compiler_output_async(
        &self,
        address: Address,
    ) -> Result<Option<Arc<CompilerOutput>>, Error> {
        // check cache first
        let output = self.compiler_output_cache.get(&address);
        if let Some(output) = output {
            return Ok(Some(output));
        }

        // no cache, compile
        // compile from compiler input cache
        let (version, input) =
            match self.get_compiler_version_and_input_async(address).await? {
                Some((version, input)) => (version, input),
                None => return Ok(None),
            };

        // check cache again
        let output = self.compiler_output_cache.get(&address);
        if let Some(output) = output {
            return Ok(Some(output));
        }

        // compile
        let version_str = format!(
            "{}.{}.{}",
            version.major, version.minor, version.patch
        );
        let compiler = Solc::find_or_install_svm_version(version_str)
        .map_err(Error::Solc)?;
        let output = compiler.compile_exact(&input).map_err(Error::Solc)?;
        let output = Arc::new(output);
        self.compiler_output_cache.insert(address, output.clone());

        Ok(Some(output))
    }

    pub async fn get_compiler_version_and_input_async(
        &self,
        address: Address,
    ) -> Result<Option<(Version, Arc<CompilerInput>)>, Error> {
        let (version, input) = {
            let input = self.compiler_input_cache.get(&address);
            if let Some(cache) = input {
                cache
            } else {
                // check if the contract's code is in the knowledge database
                let model = self.get_model_async(address).await?;
                let model = if let Some(model) = model {
                    if !model.verified {
                        return Ok(None);
                    }
                    model
                } else {
                    return Ok(None);
                };
                let input = model.compiler_input();
                let input = Arc::new(input);
                let compiler_version = model.compiler_version();
                self.compiler_input_cache
                    .insert(address, (compiler_version.clone(), input.clone()));
                (compiler_version, input)
            }
        };
        Ok(Some((version, input)))
    }

    pub async fn get_verified_code_async(
        &self,
        address: Address,
    ) -> Result<Option<Arc<Metadata>>, Error> {
        // check cache first
        let meta = self.source_code_cache.get(&address);
        if let Some(meta) = meta {
            return Ok(Some(meta));
        }

        // no cache, fetch
        let maybe_meta: Option<Metadata> = self
            .fetcher
            .fetch_verified_code(address)
            .await
            .map(Some)
            .or_else(|e| match e {
                EtherscanError::ContractCodeNotVerified(_) => Ok(None),
                _ => Err(e),
            })
            .map_err(Error::Etherscan)?;
        if let Some(meta) = maybe_meta {
            let meta = Arc::new(meta);
            self.source_code_cache.insert(address, meta.clone());
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }

    pub async fn get_model_async(
        &self,
        address: Address,
    ) -> Result<Option<Arc<entities::code::Model>>, Error> {
        // check cache first
        let model = self.model_cache.get(&address);
        if let Some(model) = model {
            return Ok(Some(model));
        }

        // try load from database
        let r = entities::code::Entity::find_by_id(address.to_string())
            .one(&self.db)
            .await;
        if let Ok(Some(m)) = r {
            let m: Arc<entities::code::Model> = Arc::new(m);
            self.model_cache.insert(address, m.clone());
            if !self.eager || m.verified {
                return Ok(Some(m));
            }
        }

        // try fetch from block explorer
        let data = self.get_verified_code_async(address).await?;
        let model = if let Some(data) = data {
            let mut model = {
                let data = data.clone();
                entities::code::Model {
                    contract: address.to_string(),
                    verified: true,
                    source: serde_json::Value::Null, // to fill
                    language: "".to_string(),        // to fill
                    abi: serde_json::Value::Null,    // to fill
                    compiler_version: data.compiler_version.clone(),
                    deployment_code: "".to_string(), // to fill
                    storage_layout: serde_json::Value::Null, // to fill
                    name: data.contract_name.clone(),
                    settings: serde_json::Value::Null, // to fill
                    constructor_args: data.constructor_arguments.to_string(),
                    license: if data.license_type.is_empty()
                        || data.license_type.to_lowercase() == "none"
                    {
                        None
                    } else {
                        Some(data.license_type.clone())
                    },
                    proxy: data.proxy > 0,
                    implementation: data.implementation.map(|s| s.to_string()),
                    swarm_source: if data.swarm_source.is_empty() {
                        None
                    } else {
                        Some(data.swarm_source.clone())
                    },
                }
            };
            let mut settings =
                data.settings().expect("failed to parse settings");
            settings.output_selection =
                OutputSelection::complete_output_selection();
            {
                // workaround for https://github.com/foundry-rs/compilers/issues/47
                settings.remappings = settings
                    .remappings
                    .into_iter()
                    .map(|mut m| {
                        m.name = if !m.name.ends_with("/") {
                            m.name.push('/');
                            m.name
                        } else {
                            m.name
                        };
                        m
                    })
                    .collect();
            }
            model.settings = serde_json::to_value(&settings)
                .expect("failed to serialize settings");

            // prepare compiler
            if data.compiler_version.starts_with("vyper") {
                model.source =
                    serde_json::Value::String(data.source_code.source_code());
                // Vyper's ABI does not fit into JsonAbi, yet.
                model.abi = serde_json::Value::String(data.abi.clone());
                model.language = "Vyper".to_string();
                let model = Arc::new(model);
                self.model_cache.insert(address, model.clone());
                return Ok(Some(model));
            } else {
                model.language = "Solidity".to_string();
                match JsonAbi::from_json_str(&data.abi) {
                    Ok(abi) => {
                        model.abi = serde_json::to_value(abi)
                            .expect("failed to serialize ABI");
                    }
                    Err(_) => {
                        // many ancient contract ABIs do not fit into JsonAbi, due to missing some fields, such as "stateMutability"
                        model.abi = serde_json::Value::String(data.abi.clone());
                    }
                };
            }

            let sources: Sources = data
                .source_code
                .sources()
                .into_iter()
                .map(|(name, source)| {
                    (
                        PathBuf::from(name),
                        Source {
                            content: Arc::new(source.content),
                        },
                    )
                })
                .collect::<Sources>();
            model.source = serde_json::to_value(&sources)
                .expect("failed to serialize sources");

            let compiler_version = if data.compiler_version.starts_with("v") {
                data.compiler_version[1..].to_string()
            } else {
                data.compiler_version.clone()
            };
            let version = Version::parse(&compiler_version)
                .expect("failed to parse version");
            let version_str = format!(
                "{}.{}.{}",
                version.major, version.minor, version.patch
            );
            if let Ok(compiler) = Solc::find_or_install_svm_version(version_str)
            .map_err(Error::Solc)
            {
                // compile
                let input = CompilerInput {
                    language: "Solidity".to_string(),
                    sources,
                    settings,
                };
                let output =
                    compiler.compile_exact(&input).map_err(Error::Solc)?;
                let output = Arc::new(output);
                self.compiler_output_cache.insert(address, output.clone());

                // handle errors
                let errs = output
                    .errors
                    .iter()
                    .filter(|e| e.severity.is_error())
                    .map(|e| e.to_owned())
                    .collect::<Vec<_>>();
                if !errs.is_empty() {
                    return Err(Error::CompilationFailed(errs));
                }

                // obtain storage layout
                let contracts = output
                    .contracts
                    .values()
                    .map(|f| {
                        f.into_iter()
                            .map(|(n, m)| (n.clone(), m.clone()))
                            .collect::<Vec<(String, Contract)>>()
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                let (_, contract_meta) = contracts
                    .iter()
                    .find(|(name, _)| *name == data.contract_name)
                    .or(contracts.last())
                    .expect("main contract not found");
                model.storage_layout =
                    serde_json::to_value(&contract_meta.storage_layout)
                        .expect("failed to serialize storage layout");
                model.deployment_code = contract_meta
                    .get_bytecode_bytes()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
            };
            let model = Arc::new(model);
            self.model_cache.insert(address, model.clone());
            model
        } else {
            let model = entities::code::Model {
                contract: address.to_string(),
                verified: false,
                ..Default::default()
            };
            Arc::new(model)
        };

        let active_model = entities::code::ActiveModel::from((*model).clone());
        match entities::code::Entity::insert(active_model)
            .on_conflict(
                OnConflict::column(entities::code::Column::Contract)
                    .update_columns(vec![
                        entities::code::Column::Verified,
                        entities::code::Column::Source,
                        entities::code::Column::Language,
                        entities::code::Column::Abi,
                        entities::code::Column::CompilerVersion,
                        entities::code::Column::DeploymentCode,
                        entities::code::Column::StorageLayout,
                        entities::code::Column::Name,
                        entities::code::Column::Settings,
                        entities::code::Column::ConstructorArgs,
                        entities::code::Column::License,
                        entities::code::Column::Proxy,
                        entities::code::Column::Implementation,
                        entities::code::Column::SwarmSource,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                if let sea_orm::error::DbErr::RecordNotInserted = e {
                    // ignore
                } else {
                    return Err(Error::Database(e));
                }
            }
        };
        if model.verified {
            Ok(Some(model))
        } else {
            Ok(None)
        }
    }
}
