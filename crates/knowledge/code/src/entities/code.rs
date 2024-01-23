use std::collections::BTreeMap;

use alloy_json_abi::JsonAbi;
use foundry_compilers::{
    artifacts::{output_selection::OutputSelection, Settings},
    CompilerInput, CompilerOutput,
};
use libsofl_core::engine::types::FixedBytes;
use sea_orm::entity::prelude::*;
use semver::Version;

use crate::error::Error;

#[derive(Clone, Default, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "code")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contract: String,

    /// verified on Etherscan
    pub verified: bool,

    /// source code
    pub source: serde_json::Value,

    /// language
    pub language: String,

    /// ABI
    pub abi: serde_json::Value,

    /// Storage layout
    pub storage_layout: serde_json::Value,

    /// contract name
    pub name: String,

    /// compiler version
    pub compiler_version: String,

    /// compiler settings
    pub settings: serde_json::Value,

    /// The bytecode to deploy the contract
    pub deployment_code: String,

    /// constructor arguments (hex encoded)
    pub constructor_args: String,

    /// license type
    pub license: Option<String>,

    /// proxy
    pub proxy: bool,

    /// implementation
    pub implementation: Option<String>,

    /// Swarm source
    pub swarm_source: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn compiler_version(&self) -> Version {
        let compiler_version: &str = if self.compiler_version.starts_with("v") {
            &self.compiler_version[1..]
        } else {
            self.compiler_version.as_str()
        };
        Version::parse(compiler_version).expect("invalid version")
    }

    pub fn abi(&self) -> JsonAbi {
        let json_str = serde_json::to_string(&self.abi).expect("invalid ABI");
        JsonAbi::from_json_str(&json_str).expect("invalid ABI")
    }

    pub fn function_signatures(&self) -> BTreeMap<FixedBytes<4>, String> {
        let abi = self.abi();
        let signatures = abi
            .functions
            .values()
            .flatten()
            .map(|f| (f.selector(), f.full_signature()))
            .collect::<BTreeMap<_, _>>();
        return signatures;
    }

    pub fn compiler_input(&self) -> CompilerInput {
        let sources = serde_json::from_value(self.source.clone())
            .expect("invalid source");
        let mut settings: Settings =
            serde_json::from_value(self.settings.clone())
                .expect("invalid settings");
        settings.output_selection =
            OutputSelection::complete_output_selection();
        CompilerInput {
            language: self.language.clone(),
            sources,
            settings,
        }
    }

    pub fn compile(&self) -> Result<CompilerOutput, Error> {
        let version = self.compiler_version();
        let compiler = foundry_compilers::Solc::find_or_install_svm_version(
            format!("{}.{}.{}", version.major, version.minor, version.patch)
                .as_str(),
        )
        .map_err(Error::Solc)?;
        let input = self.compiler_input();
        compiler.compile_exact(&input).map_err(Error::Solc)
    }
}
