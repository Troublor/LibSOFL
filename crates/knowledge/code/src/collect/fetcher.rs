use crate::config::CodeKnowledgeConfig;
use crate::entities;
use crate::error::Error;
use alloy_json_abi::JsonAbi;
use foundry_block_explorers::errors::EtherscanError;
use foundry_block_explorers::Client;
use foundry_compilers::artifacts::output_selection::OutputSelection;
use foundry_compilers::artifacts::{Contract, Source, Sources};
use foundry_compilers::{Artifact, CompilerInput, Solc};
use libsofl_core::engine::types::Address;
use libsofl_utils::rate_limit::RateLimit;
use regex::Regex;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub struct Fetcher {
    pub client: Client,
    pub rate_limit: RwLock<RateLimit>,
}

impl Fetcher {
    pub fn new(cfg: CodeKnowledgeConfig) -> Result<Self, Error> {
        let client = cfg.get_client().map_err(Error::Etherscan)?;
        let rate_limit = if let Some(freq) = cfg.requests_per_second {
            RateLimit::new_frequency(freq)
        } else {
            RateLimit::unlimited()
        };
        Ok(Self {
            client,
            rate_limit: RwLock::new(rate_limit),
        })
    }
}

impl Fetcher {
    pub async fn fetch_one(
        &self,
        address: Address,
    ) -> Result<entities::code::Model, Error> {
        self.rate_limit
            .write()
            .unwrap()
            .wait_and_increment_async()
            .await;

        let mut data = match self.client.contract_source_code(address).await {
            Err(EtherscanError::ContractCodeNotVerified(_)) => {
                return Ok(entities::code::Model {
                    contract: address.to_string(),
                    verified: false,
                    ..Default::default()
                });
            }
            Err(e) => return Err(Error::Etherscan(e)),
            Ok(meta) => meta,
        };
        assert_eq!(data.items.len(), 1, "expected one item");
        let data = data.items.remove(0);

        let mut model = {
            let data = data.clone();
            entities::code::Model {
                contract: address.to_string(),
                verified: true,
                source: serde_json::Value::Null, // to fill
                language: "".to_string(),        // to fill
                abi: serde_json::Value::Null,    // to fill
                compiler_version: data.compiler_version,
                deployment_code: "".to_string(), // to fill
                storage_layout: serde_json::Value::Null, // to fill
                name: data.contract_name,
                settings: serde_json::Value::Null, // to fill
                constructor_args: data.constructor_arguments.to_string(),
                license: if data.license_type.is_empty()
                    || data.license_type.to_lowercase() == "none"
                {
                    None
                } else {
                    Some(data.license_type)
                },
                proxy: data.proxy > 0,
                implementation: data.implementation.map(|s| s.to_string()),
                swarm_source: if data.swarm_source.is_empty() {
                    None
                } else {
                    Some(data.swarm_source)
                },
            }
        };
        let mut settings = data.settings().expect("failed to parse settings");
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
            model.abi = serde_json::Value::String(data.abi);
            model.language = "Vyper".to_string();
            return Ok(model);
        } else {
            model.language = "Solidity".to_string();
            match JsonAbi::from_json_str(&data.abi) {
                Ok(abi) => {
                    model.abi = serde_json::to_value(abi)
                        .expect("failed to serialize ABI");
                }
                Err(_) => {
                    // many ancient contract ABIs do not fit into JsonAbi, due to missing some fields, such as "stateMutability"
                    model.abi = serde_json::Value::String(data.abi);
                }
            };
        }
        let regex = Regex::new(r"v?(\d+\.\d+\.\d+)(\+.*)?")
            .expect("failed to compile regex");
        let captures = regex
            .captures(&data.compiler_version)
            .expect("failed to capture compiler version");
        let version = captures.get(1).expect("invalid version").as_str();
        let compiler =
            Solc::find_or_install_svm_version(version).map_err(Error::Solc)?;

        // compile
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
        let input = CompilerInput {
            language: "Solidity".to_string(),
            sources,
            settings,
        };
        let output = compiler.compile_exact(&input).map_err(Error::Solc)?;

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

        Ok(model)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_fetch_ancient_contract() {
        let cfg = crate::config::CodeKnowledgeConfig {
            chain_id: 1,
            api_key: "".to_string(),
            requests_per_second: None,
        };
        let fetcher = super::Fetcher::new(cfg).unwrap();
        let address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2" // WETH
            .parse()
            .unwrap();
        let model = fetcher.fetch_one(address).await.unwrap();
        println!("{:#?}", model);
        assert_eq!(
            model.contract,
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_fetch_modern_multi_file_contract() {
        let cfg = crate::config::CodeKnowledgeConfig {
            chain_id: 1,
            api_key: "".to_string(),
            requests_per_second: None,
        };
        let fetcher = super::Fetcher::new(cfg).unwrap();
        let address = "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD" // Uniswap Universal Router
            .parse()
            .unwrap();
        let model = fetcher.fetch_one(address).await.unwrap();
        println!("{:#?}", model);
        assert_eq!(
            model.contract,
            "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_fetch_proxy() {
        let cfg = crate::config::CodeKnowledgeConfig {
            chain_id: 1,
            api_key: "".to_string(),
            requests_per_second: None,
        };
        let fetcher = super::Fetcher::new(cfg).unwrap();
        let address = "0xDef1C0ded9bec7F1a1670819833240f027b25EfF" // 0x: Exchange Proxy
            .parse()
            .unwrap();
        let model = fetcher.fetch_one(address).await.unwrap();
        println!("{:#?}", model);
        assert_eq!(
            model.contract,
            "0xDef1C0ded9bec7F1a1670819833240f027b25EfF"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_fetch_vyper_contract() {
        let cfg = crate::config::CodeKnowledgeConfig {
            chain_id: 1,
            api_key: "".to_string(),
            requests_per_second: None,
        };
        let fetcher = super::Fetcher::new(cfg).unwrap();
        let address = "0xA2B47E3D5c44877cca798226B7B8118F9BFb7A56" // Curve.fi Compound Swap
            .parse()
            .unwrap();
        let model = fetcher.fetch_one(address).await.unwrap();
        println!("{:#?}", model);
        assert_eq!(
            model.contract,
            "0xA2B47E3D5c44877cca798226B7B8118F9BFb7A56"
        );
        assert_eq!(model.language, "Vyper");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_frequency_control() {
        let cfg = crate::config::CodeKnowledgeConfig {
            chain_id: 1,
            api_key: "".to_string(),
            requests_per_second: Some(0.1),
        };
        let fetcher = super::Fetcher::new(cfg).unwrap();
        let address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2" // WETH
            .parse()
            .unwrap();
        let start = Instant::now();
        for _ in 0..3 {
            let address = address;
            fetcher.fetch_one(address).await.unwrap();
            println!("elapsed: {:?}", start.elapsed());
        }
        let elapsed = start.elapsed();
        assert!(elapsed > Duration::from_secs(20));
    }
}
