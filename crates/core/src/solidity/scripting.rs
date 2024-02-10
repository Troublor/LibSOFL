use std::path::PathBuf;

use crate::{
    conversion::ConvertTo,
    engine::{
        inspector::no_inspector,
        state::BcState,
        types::{Address, BlockEnv, Bytes, CfgEnv, SpecId, U256},
    },
    error::SoflError,
};
use alloy_dyn_abi::JsonAbiExt;
use alloy_json_abi::Function;
pub use alloy_sol_types::sol_data;
use foundry_compilers::{
    artifacts::{Error, Source, Sources},
    CompilerInput, Solc,
};
use tracing::error;

use super::caller::HighLevelCaller;

#[derive(Debug, Clone, Default)]
pub struct SolScriptConfig {
    /// The address of the deployer. If not set, the caller address is used.
    pub deployer: Option<Address>,

    pub salt: Option<U256>,

    /// The amount of ether transferred to the contract when during deployment.
    pub prefund: U256,

    /// The address of the caller calling run() function. If not set, a random address is used.
    pub caller: Option<Address>,

    /// The amount of ether transferred to the contract when calling run() function.
    pub value: U256,

    /// The gas limit of run() function.
    pub gas_limit: Option<u64>,

    pub block: BlockEnv,
    pub cfg: CfgEnv,
}

/// Run a solidity script.
/// The script is essentially a contract whose name has `Script` suffix and has a `run() public` function.
/// This function will:
/// 1. compile the script contract.
/// 2. deploy the script contract.
/// 3. call the `run()` function of the script contract.
/// The `run()` function may have return values, but the parse of return value is left to be done by the caller.
pub fn run_sol<S: BcState>(
    mut state: S,
    solidity_version: &str,
    code: impl ToString,
    config: SolScriptConfig,
) -> Result<Bytes, SoflError> {
    // compile
    let contracts = compile_solidity(solidity_version, code)?;
    let (_, bytecode) = contracts
        .iter()
        .filter(|(n, _)| n.ends_with("Script"))
        .next()
        .expect("script contract not found");

    let caller = config.caller.unwrap_or_default();
    let deployer = config.deployer.unwrap_or(caller);

    // deploy the contract
    state.add_ether_balance(deployer, config.prefund)?;
    let h_caller = HighLevelCaller::new(deployer)
        .bypass_check()
        .set_evm_version(SpecId::LATEST)
        .set_block(config.block.clone())
        .set_cfg(config.cfg.clone());
    let (_, addr) = h_caller.create(
        &mut state,
        config.salt,
        bytecode,
        Some(config.prefund),
        no_inspector(),
    )?;
    let addr = addr.expect("impossible: address is none");

    // execute the contract's run function
    let func = Function::parse("run()").expect("failed to parse function");
    let input: Bytes = func
        .abi_encode_input(&[])
        .expect("failed to encode input")
        .cvt();
    let h_caller = h_caller.set_address(caller);
    let ret = h_caller
        .set_gas_limit(config.gas_limit.unwrap_or(u64::MAX))
        .call(&mut state, addr, input, Some(config.value), no_inspector())?;

    Ok(ret)
}

/// Deploy a solidity contract.
/// There may be multiple contracts in the solidity code, but only the contracts with names in `contract_names` will be deployed.
/// If any contract name does not exists, this function will panic.
pub fn deploy_contracts<S: BcState>(
    mut state: S,
    solidity_version: &str,
    contract_code: impl ToString,
    contract_names: Vec<&str>,
    config: SolScriptConfig,
) -> Result<Vec<Address>, SoflError> {
    let contracts = compile_solidity(solidity_version, contract_code)?;
    let deployer = config.deployer.unwrap_or_default();
    let mut addresses = Vec::new();
    for n in contract_names {
        let (_, bytecode) = contracts
            .iter()
            .find(|(name, _)| name == &n)
            .expect(format!("no contract named {} found", n).as_str());
        state.add_ether_balance(deployer, config.prefund)?;
        let (_, addr) = HighLevelCaller::new(deployer)
            .bypass_check()
            .set_evm_version(SpecId::LATEST)
            .set_block(config.block.clone())
            .set_cfg(config.cfg.clone())
            .create(
                &mut state,
                config.salt,
                &bytecode,
                Some(config.prefund),
                no_inspector(),
            )?;
        let addr = addr.expect("impossible: address is none");
        addresses.push(addr);
    }
    Ok(addresses)
}

/// Compile a solidity string.
/// Return a map from contract name to its deployment bytecode.
pub fn compile_solidity(
    compiler_version: &str,
    code: impl ToString,
) -> Result<Vec<(String, Bytes)>, SoflError> {
    compile_contract(compiler_version, code, false)
}

pub fn compile_yul(
    compiler_version: &str,
    code: impl ToString,
) -> Result<Vec<(String, Bytes)>, SoflError> {
    compile_contract(compiler_version, code, true)
}

pub fn compile_contract(
    compiler_version: &str,
    contract_code: impl ToString,
    is_yul: bool,
) -> Result<Vec<(String, Bytes)>, SoflError> {
    // prepare compiler input
    let compiler = Solc::find_or_install_svm_version(compiler_version)
        .expect("solc version not found");
    let version = compiler.version().expect("failed to get solc version");
    let source = Source::new(contract_code.to_string());
    let mut sources = Sources::new();
    let file = if is_yul { "code.yul" } else { "code.sol" };
    sources.insert(PathBuf::from(file), source);
    let compiler_input = CompilerInput::with_sources(sources)
        .remove(0)
        .normalize_evm_version(&version);

    // compile
    let compiler_output = compiler
        .compile_exact(&compiler_input)
        .expect("failed to compile contract code");
    let errs: Vec<Error> = compiler_output
        .errors
        .iter()
        .filter(|e| e.severity.is_error())
        .map(|e| e.to_owned())
        .collect();
    if errs.len() > 0 {
        error!(errors = errs.len(), "failed to compile code",);
        for e in errs {
            eprintln!("{}", e);
        }
        Err(SoflError::Custom("failed to compile yul code".to_string()))
    } else {
        let contracts = compiler_output
            .contracts
            .get(file)
            .expect("file not found in compiler output")
            .into_iter()
            .map(|(n, c)| {
                let c = c
                    .evm
                    .to_owned()
                    .expect("evm field not found in compiler output");
                let bytecode = c
                    .bytecode
                    .expect("bytecode field not found in compiler output");
                (
                    n.clone(),
                    bytecode
                        .object
                        .into_bytes()
                        .expect("failed to convert bytecode to bytes"),
                )
            })
            .collect();
        Ok(contracts)
    }
}

#[derive(Debug, Clone, Default)]
pub struct YulScriptConfig {
    pub contract: Address, // under the context of which the yul code is executed
    pub caller: Address,
    pub value: U256,
    pub data: Bytes,
    pub gas_limit: Option<u64>,
    pub block: BlockEnv,
    pub cfg: CfgEnv,
}

/// Run yul code under the state of a contract.
/// The code of the config.contract will be first replaced to the yul code,
/// and then the yul code will be executed with the env speicified in config.
/// Note that if there are multiple yul contract in the code, only the first one will be executed.
pub fn run_yul<S: BcState>(
    mut state: S,
    compiler_version: &str,
    code: &str,
    config: YulScriptConfig,
) -> Result<Bytes, SoflError> {
    let (_, bytecode) = compile_yul(compiler_version, code)?.remove(0);
    let original_code = state
        .basic(config.contract)
        .unwrap()
        .unwrap_or_default()
        .code
        .unwrap_or_default();
    state.replace_account_code(config.contract, bytecode.cvt())?;
    let ret = HighLevelCaller::new(config.caller)
        .bypass_check()
        .set_evm_version(SpecId::LATEST)
        .set_block(config.block)
        .set_cfg(config.cfg)
        .call(
            &mut state,
            config.contract,
            config.data,
            Some(config.value),
            no_inspector(),
        )?;
    state.replace_account_code(config.contract, original_code)?;
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use crate::{
        conversion::ConvertTo,
        engine::{
            inspector::no_inspector,
            memory::MemoryBcState,
            types::{Address, Database, SpecId},
        },
    };
    use alloy_dyn_abi::JsonAbiExt;
    use alloy_json_abi::Function;
    use alloy_sol_types::{sol_data, SolType};

    use crate::solidity::{
        caller::HighLevelCaller, scripting::SolScriptConfig,
    };

    use super::{deploy_contracts, run_sol, run_yul, YulScriptConfig};

    #[test]
    pub fn test_run_sol_simple() {
        let mut state = MemoryBcState::fresh();
        let ret = run_sol(
            &mut state,
            "0.8.12",
            r#"
            contract Script {
                function run() public returns (uint256) {
                    uint256 a = 1;
                    uint256 b = 2;
                    uint256 c = a + b;
                    return c;
                }
            }
        "#,
            Default::default(),
        )
        .unwrap();
        let ret = sol_data::Uint::<256>::abi_decode(&ret, true).unwrap();
        assert_eq!(ConvertTo::<usize>::cvt(&ret), 3);
    }

    #[test]
    pub fn test_transfer_ether() {
        let mut state = MemoryBcState::fresh();
        let receiver: Address = 0x1234567890abcdef.cvt();
        let code = format!(
            r#"
                contract Script {{
                    constructor() payable {{ }}
                    function run() public {{
                        address to = {};
                        require(address(this).balance >= 1 ether, "insufficient balance");
                        payable(to).transfer(1 ether);
                    }}
                }}
            "#,
            receiver,
        );
        let _ = run_sol(
            &mut state,
            "0.8.12",
            code,
            SolScriptConfig {
                prefund: 1_000_000_000_000_000_000u128.cvt(),
                ..Default::default()
            },
        )
        .unwrap();
        let balance = state.basic(receiver).unwrap().unwrap().balance;
        assert_eq!(balance, 1_000_000_000_000_000_000u128.cvt());
    }

    #[test]
    fn test_deploy_another_contract_in_script() {
        let mut state = MemoryBcState::fresh();
        let code = r#"
                contract Script {
                    constructor() payable {}
                    function run() public returns (address) {
                        address addr = address(new D());
                        return addr;
                    }
                }
                contract D {
                    function hello() public returns (string memory) {
                        return "world";
                    }
                }
            "#;
        let ret =
            run_sol(&mut state, "0.8.12", code, Default::default()).unwrap();
        let contract = sol_data::Address::abi_decode(&ret, true).unwrap();

        let input = Function::parse("hello()")
            .unwrap()
            .abi_encode_input(&[])
            .unwrap()
            .cvt();
        let ret = HighLevelCaller::default()
            .bypass_check()
            .set_evm_version(SpecId::LATEST)
            .call(&mut state, contract, input, None, no_inspector())
            .unwrap();
        let ret = sol_data::String::abi_decode(&ret, true).unwrap();
        assert_eq!(ret, "world");
    }

    #[test]
    fn test_only_deploy_first() {
        let mut state = MemoryBcState::fresh();
        let code = r#"
                contract First {
                    string public name = "first";
                }
                contract Second {
                    string public name = "second";
                }
            "#;
        let addr = deploy_contracts(
            &mut state,
            "0.8.12",
            code,
            vec!["First"],
            Default::default(),
        )
        .unwrap()
        .remove(0);
        let input = Function::parse("name()")
            .unwrap()
            .abi_encode_input(&[])
            .unwrap()
            .cvt();
        let ret = HighLevelCaller::default()
            .bypass_check()
            .set_evm_version(SpecId::LATEST)
            .call(&mut state, addr, input, None, no_inspector())
            .unwrap();
        let ret = sol_data::String::abi_decode(&ret, true).unwrap();
        assert_eq!(ret, "first");
    }

    #[test]
    fn test_run_yul() {
        let mut state = MemoryBcState::fresh();
        let code = r#"
                contract A {
                    uint public x = 1;
                }
            "#;
        let addr = deploy_contracts(
            &mut state,
            "0.8.12",
            code,
            vec!["A"],
            Default::default(),
        )
        .unwrap()
        .remove(0);

        let yul = r#"
            object "A" {
                code {
                    let slot := 0x00
                    let value := sload(slot)
                    value := add(value, 1)
                    mstore(0x00, value)
                    return (0x00, 0x20)
                }
            }
        "#;
        let ret = run_yul(
            &mut state,
            "0.8.12",
            yul,
            YulScriptConfig {
                contract: addr,
                ..Default::default()
            },
        )
        .unwrap();
        let ret = sol_data::Uint::<256>::abi_decode(&ret, true).unwrap();
        assert_eq!(ConvertTo::<usize>::cvt(&ret), 2);

        // the code of the contract should remain unchanged
        let ret = HighLevelCaller::default()
            .bypass_check()
            .set_evm_version(SpecId::LATEST)
            .view(&mut state, addr, "x() returns (uint)", &[], no_inspector())
            .unwrap()
            .remove(0);
        assert_eq!(ConvertTo::<usize>::cvt(&ret.as_uint().unwrap().0), 1);
    }
}
