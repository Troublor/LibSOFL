use foundry_compilers::{
    artifacts::{output_selection::OutputSelection, Settings},
    CompilerInput, CompilerOutput,
};
use regex::Regex;
use sea_orm::entity::prelude::*;

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
    pub fn compiler_version(&self) -> String {
        let regex = Regex::new(r"v?(\d+\.\d+\.\d+)(\+.*)?")
            .expect("failed to compile regex");
        let captures = regex
            .captures(&self.compiler_version)
            .expect("failed to capture compiler version");
        let version = captures.get(1).expect("invalid version").as_str();
        version.to_string()
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
        let compiler =
            foundry_compilers::Solc::find_or_install_svm_version(&version)
                .map_err(Error::Solc)?;
        let input = self.compiler_input();
        compiler.compile_exact(&input).map_err(Error::Solc)
    }
}
