pub mod service;

use std::{collections::BTreeMap, sync::Arc};

use alloy_json_abi::JsonAbi;
use foundry_compilers::{
    artifacts::StorageLayout, CompilerInput, CompilerOutput,
};
use jsonrpsee::{core::async_trait, proc_macros::rpc};
use libsofl_core::engine::types::{Address, FixedBytes};
use semver::Version;

use crate::{error::Error, query::query::CodeQuery};

#[rpc(client, server, namespace = "kb")]
pub trait CodeRpc {
    #[method(name = "compilerInput")]
    async fn compiler_input(
        &self,
        address: Address,
    ) -> Result<Option<(Version, CompilerInput)>, Error>;

    #[method(name = "compilerOutput")]
    async fn compiler_output(
        &self,
        address: Address,
    ) -> Result<Option<CompilerOutput>, Error>;

    #[method(name = "compilerVersion")]
    async fn compiler_version(
        &self,
        address: Address,
    ) -> Result<Option<Version>, Error>;

    #[method(name = "storageLayout")]
    async fn storage_layout(
        &self,
        address: Address,
    ) -> Result<Option<StorageLayout>, Error>;

    #[method(name = "abi")]
    async fn abi(&self, address: Address) -> Result<Option<JsonAbi>, Error>;

    #[method(name = "functionSignatures")]
    async fn function_signatures(
        &self,
        address: Address,
    ) -> Result<Option<BTreeMap<FixedBytes<4>, String>>, Error>;
}

pub struct CodeRpcImpl {
    pub query: Arc<CodeQuery>,
}

#[async_trait]
impl CodeRpcServer for CodeRpcImpl {
    async fn compiler_input(
        &self,
        address: Address,
    ) -> Result<Option<(Version, CompilerInput)>, Error> {
        self.query
            .get_compiler_version_and_input_async(address)
            .await
            .map(|x| x.map(|(v, i)| (v, (*i).clone())))
    }

    async fn compiler_output(
        &self,
        address: Address,
    ) -> Result<Option<CompilerOutput>, Error> {
        self.query
            .get_compiler_output_async(address)
            .await
            .map(|x| x.map(|o| (*o).clone()))
    }

    async fn compiler_version(
        &self,
        address: Address,
    ) -> Result<Option<Version>, Error> {
        self.query
            .get_compiler_version_and_input_async(address)
            .await
            .map(|x| x.map(|v| v.0))
    }

    async fn storage_layout(
        &self,
        address: Address,
    ) -> Result<Option<StorageLayout>, Error> {
        self.query
            .get_storage_layout_async(address)
            .await
            .map(|x| x.map(|l| (*l).clone()))
    }

    async fn abi(&self, address: Address) -> Result<Option<JsonAbi>, Error> {
        self.query
            .get_abi_async(address)
            .await
            .map(|x| x.map(|a| (*a).clone()))
    }

    async fn function_signatures(
        &self,
        address: Address,
    ) -> Result<Option<BTreeMap<FixedBytes<4>, String>>, Error> {
        self.query
            .get_function_signatures_async(address)
            .await
            .map(|x| x.map(|s| (*s).clone()))
    }
}
