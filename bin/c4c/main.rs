use clap::{Parser, Subcommand};
use libsofl::engine::{
    inspectors::no_inspector,
    providers::{BcProvider, BcProviderBuilder},
    state::{env::TransitionSpecBuilder, BcState, BcStateBuilder},
    transactions::position::TxPosition,
    utils::HighLevelCaller,
};
use reth_primitives::TxHash;
use revm::Database;
use revm_primitives::{hex, Address, Bytecode, ExecutionResult, B256};
use serde::Deserialize;
use std::{fs, path::PathBuf, str::FromStr};

#[derive(Parser)]
#[command(name = "C4C-Simulator")]
#[command(version = "0.1")]
#[command(about = "Utilities for Project C4C", long_about = None)]
struct Cli {
    /// Deploy Configuration
    #[arg(long)]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    // Get Deployed Bytecode
    Deploy {},
    Reproduce {
        #[arg(long)]
        tx_hash: String,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    creation_code: String,
    deployer: String,
    block: u64,
    replacee_address: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let bp = BcProviderBuilder::default_db().unwrap();

    let config_file = cli.config;
    let config_data = fs::read_to_string(config_file)
        .expect("Something went wrong reading the file");
    let config: Config =
        toml::from_str(&config_data).expect("Unable to parse the TOML");

    match &cli.command {
        Some(Commands::Deploy {}) => {
            let (code_hash, _) = deploy(&bp, &config);
            println!("New Code Hash: {:?}", code_hash);
        }
        Some(Commands::Reproduce { tx_hash }) => {
            let tx_hash = TxHash::from_str(tx_hash).unwrap();
            let gas_usage = replay(bp, &config, tx_hash);
            println!("Gas Usage: {:?}", gas_usage);
        }
        None => {
            println!("No command specified");
        }
    }
}

fn replay<P: BcProvider>(bp: P, config: &Config, tx_hash: TxHash) -> u64 {
    let (tx, tx_meta) =
        bp.transaction_by_hash_with_meta(tx_hash).unwrap().unwrap();
    let block = TxPosition::new(tx_meta.block_number, tx_meta.index);

    if let Some(ref addr_str) = config.replacee_address {
        let addr = Address::from_str(addr_str).unwrap();
        println!(
            "Try to replace the code of {} and reproduce {}",
            addr, tx_hash
        );

        let mut state = BcStateBuilder::fork_at(&bp, block).unwrap();

        let (code_hash, bytecode) = deploy(&bp, config);

        let mut account_info = state.basic(addr).unwrap().unwrap();
        if account_info.code_hash != code_hash {
            println!(
                "Replacee Code Changed: {:?}",
                account_info.code_hash != code_hash
            );
            account_info.code_hash = code_hash;
            account_info.code = Some(bytecode);
            state.insert_account_info(addr, account_info);
        }

        let mut spec_builder =
            TransitionSpecBuilder::new().at_block(&bp, block.block);
        spec_builder = spec_builder.append_signed_tx(tx);
        let spec = spec_builder.build();

        let (_, results) =
            BcState::transit(state, spec, no_inspector()).unwrap();
        let result = results[0].clone();
        match result {
            ExecutionResult::Success { gas_used, .. } => gas_used,
            _ => panic!("Transaction failed"),
        }
    } else {
        let receipts = bp.receipts_by_block(block.block).unwrap().unwrap();
        if block.index == 0 {
            receipts[0].cumulative_gas_used
        } else {
            receipts[block.index as usize].cumulative_gas_used
                - receipts[(block.index - 1) as usize].cumulative_gas_used
        }
    }
}

fn deploy<P: BcProvider>(bp: &P, config: &Config) -> (B256, Bytecode) {
    let block = TxPosition::new(config.block, 0);
    let mut state = BcStateBuilder::fork_at(bp, block).unwrap();

    let deployer = Address::from_str(&config.deployer).unwrap();
    let caller: HighLevelCaller =
        HighLevelCaller::from(deployer).bypass_check();
    let data = hex::decode(config.creation_code.trim()).unwrap();

    let (_, addr) = caller
        .create(&mut state, &data, None, no_inspector())
        .unwrap();

    let deployed_addr = addr.unwrap();

    let code_hash = state.basic(deployed_addr).unwrap().unwrap().code_hash;
    (code_hash, state.code_by_hash(code_hash).unwrap())
}
