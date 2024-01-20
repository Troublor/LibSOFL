use crate::config::CodeKnowledgeConfig;
use crate::entities;
use crate::error::Error;
use alloy_json_abi::JsonAbi;
use foundry_block_explorers::contract::SourceCodeLanguage;
use foundry_block_explorers::Client;
use foundry_compilers::artifacts::{Contract, Source, Sources};
use foundry_compilers::{Artifact, CompilerInput, Solc};
use libsofl_core::engine::types::Address;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Fetcher {
    pub client: Client,
}

impl Fetcher {
    pub fn new(cfg: CodeKnowledgeConfig) -> Result<Self, Error> {
        let client = cfg.get_client().map_err(Error::Etherscan)?;
        Ok(Self { client })
    }
}

impl Fetcher {
    pub async fn fetch_one(
        &self,
        address: Address,
    ) -> Result<entities::code::Model, Error> {
        let mut data = self
            .client
            .contract_source_code(address)
            .await
            .map_err(Error::Etherscan)?;
        assert_eq!(data.items.len(), 1, "expected one item");
        let data = data.items.remove(0);

        let mut model = {
            let data = data.clone();
            let abi = JsonAbi::from_json_str(&data.abi).expect("invalid ABI");
            entities::code::Model {
                contract: address.to_string(),
                source: serde_json::Value::Null, // to fill
                language: "".to_string(),        // to fill
                abi: serde_json::to_value(abi)
                    .expect("failed to serialize ABI"),
                deployment_code: "".to_string(), // to fill
                storage_layout: serde_json::Value::Null, // to fill
                name: data.contract_name,
                compiler: data.compiler_version,
                optimization: if data.optimization_used > 0 {
                    Some(data.runs as i32)
                } else {
                    None
                },
                constructor_args: data.constructor_arguments.to_string(),
                evm_version: data.evm_version,
                library: if data.library.is_empty() {
                    None
                } else {
                    Some(data.library)
                },
                license: if data.license_type.is_empty() {
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

        // prepare compiler
        if data.compiler_version.starts_with("vyper") {
            model.source =
                serde_json::Value::String(data.source_code.source_code());
            let abi = self
                .client
                .contract_abi(address)
                .await
                .map_err(Error::Etherscan)?;
            model.abi =
                serde_json::to_value(abi).expect("failed to serialize ABI");
            model.language = "Vyper".to_string();
            return Ok(model);
        } else {
            model.language = "Solidity".to_string();
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
        let settings = data.settings().expect("failed to parse settings");
        let sources: Sources = data
            .source_code
            .sources()
            .into_iter()
            .map(|(name, source)| {
                println!("{}", name);
                (
                    PathBuf::from(name),
                    Source {
                        content: Arc::new(source.content),
                    },
                )
            })
            .collect::<Sources>();
        let mut allowed_paths = HashSet::new();
        let mut lib_paths = HashSet::new();
        for (path, _) in &sources {
            let path = path.parent().unwrap();
            if !allowed_paths.contains(path.to_str().unwrap()) {
                allowed_paths.insert(path.to_string_lossy());
            }
            let mut path = path;
            loop {
                let basename = match path.file_name() {
                    Some(basename) => basename.to_string_lossy(),
                    None => break,
                };
                if basename == "node_modules" {
                    lib_paths.insert(path.to_string_lossy());
                    break;
                }
                path = match path.parent() {
                    Some(path) => path,
                    None => break,
                };
            }
        }
        let allowed_paths = allowed_paths.into_iter().collect::<Vec<_>>();
        let lib_paths = lib_paths.into_iter().collect::<Vec<_>>();
        let compiler = compiler
            // .arg("--allow-paths")
            // .arg(allowed_paths.join(","))
            .arg("--include-path")
            .arg(lib_paths.join(","));

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
    #[tokio::test(flavor = "multi_thread")]
    async fn test_fetch_ancient_contract() {
        let cfg = crate::config::CodeKnowledgeConfig {
            chain_id: 1,
            api_key: "".to_string(),
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
}
