use clap::{Parser, Subcommand};
use hashbrown::HashMap;
use libsofl::{
    engine::{
        inspectors::no_inspector,
        providers::BcProviderBuilder,
        state::{env::TransitionSpecBuilder, BcState, BcStateBuilder},
        transactions::position::TxPosition,
        utils::HighLevelCaller,
    },
    utils::conversion::{Convert, ToElementary, ToPrimitive},
};
use reth_primitives::TxHash;
use reth_provider::{
    BlockHashReader, BlockNumReader, EvmEnvProvider, ReceiptProvider,
    StateProviderFactory, TransactionsProvider,
};
use revm::{Database, DatabaseCommit};
use revm_primitives::{hex, Address, Bytecode, ExecutionResult, B256, U256};
use serde::Deserialize;
use std::{collections::HashSet, fmt, fs, path::PathBuf, str::FromStr};

mod inspector;
use inspector::InternalTransactionInspector;

#[derive(Parser)]
#[command(name = "C4C-Simulator")]
#[command(version = "0.1")]
#[command(about = "Utilities for Project C4C", long_about = None)]
struct Cli {
    /// Deploy Configuration
    #[arg(long)]
    config: PathBuf,

    /// Creation code
    #[arg(long)]
    creation_code: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    // Get Deployed Bytecode
    Deploy {},
    Reproduce {
        #[arg(long)]
        tx_hash: Option<String>,
    },
    GasEstimation {
        #[arg(long)]
        start_tx_hash: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    deployer: String,
    block: u64,
    replacee_address: Option<String>,
    attack_transaction: Option<String>,
    earliest_transaction: Option<String>,
    blocks: Option<Vec<u64>>,
    excluded_transactions: Option<Vec<String>>,
    storages: Option<Vec<(u64, String)>>,
}

fn main() {
    let cli = Cli::parse();
    let bp = BcProviderBuilder::default_db().unwrap();

    let config_file = cli.config;
    let config_data = fs::read_to_string(config_file)
        .expect("Something went wrong reading the file");
    let config: Config =
        toml::from_str(&config_data).expect("Unable to parse the TOML");

    let creation_file = cli.creation_code;
    let creation_code = fs::read_to_string(creation_file)
        .expect("Something went wrong reading the file");

    match &cli.command {
        Some(Commands::Deploy {}) => {
            let (code_hash, _) = deploy(&bp, &config, &creation_code);
            println!("New Code Hash: {:?}", code_hash);
        }
        Some(Commands::Reproduce { tx_hash }) => {
            let tx_hash = if let Some(tx_hash_str) = tx_hash {
                TxHash::from_str(tx_hash_str).unwrap()
            } else {
                TxHash::from_str(
                    config
                        .attack_transaction
                        .as_ref()
                        .expect("No attack transaction specified"),
                )
                .unwrap()
            };
            if let Some((gas_usage, eth_price)) =
                replay(&bp, &config, &creation_code, tx_hash)
            {
                let eth_price: u64 = ToElementary::cvt(eth_price);
                println!(
                    "Gas Usage: {:?}, ETH Price: ${}",
                    gas_usage,
                    eth_price as f64 / 100_000_000f64
                );
            } else {
                println!("Replay Failed");
            }
        }
        Some(Commands::GasEstimation { start_tx_hash }) => {
            let start_tx_hash = if let Some(ref tx_hash_str) = start_tx_hash {
                TxHash::from_str(tx_hash_str).unwrap()
            } else {
                TxHash::from_str(config.earliest_transaction.as_ref().unwrap())
                    .unwrap()
            };
            let res =
                estimate_gas_usage(&bp, &config, &creation_code, start_tx_hash);

            let mut total_gas = 0u64;
            let mut total_gas_price = 0f64;

            println!("Gas Usage Estimation:");
            println!("TxHash, GasUsage, GasUnitPrice");
            for (tx_hash, (gas, gas_price, eth_price)) in &res {
                let gas_unit: u128 = ToElementary::cvt(
                    eth_price * gas_price / U256::from(10).pow(U256::from(18)),
                );
                let gas_unit = gas_unit as f64 / 100_000_000f64;

                println!("{:?}, {}, {}", tx_hash, gas, gas_unit);

                total_gas += gas;
                total_gas_price += gas_unit * (*gas as f64);
            }
            println!(
                "\nTotal Transactions: {}\nTotal Gas Usage: {}\nTotal Gas Usage in USD: {}",
                res.len(), total_gas, total_gas_price
            );
        }
        None => {
            println!("No command specified");
        }
    }
}

fn estimate_gas_usage<
    P: ReceiptProvider
        + TransactionsProvider
        + BlockNumReader
        + BlockHashReader
        + StateProviderFactory
        + EvmEnvProvider,
>(
    bp: &P,
    config: &Config,
    creation_code: &str,
    start_tx_hash: TxHash,
) -> HashMap<TxHash, (u64, U256, U256)> {
    let attack_tx_hash = TxHash::from_str(
        config
            .attack_transaction
            .as_ref()
            .expect("No attack transaction specified"),
    )
    .unwrap();
    let attack_block = if let Some((_, attack_tx_meta)) =
        bp.transaction_by_hash_with_meta(attack_tx_hash).unwrap()
    {
        attack_tx_meta.block_number
    } else {
        17471230
    };

    let (_, start_tx_meta) = bp
        .transaction_by_hash_with_meta(start_tx_hash)
        .unwrap()
        .unwrap();
    let start_block = start_tx_meta.block_number;

    let replacee_contract =
        Address::from_str(config.replacee_address.as_ref().unwrap()).unwrap();
    let txs = get_txs_by_block_range(
        bp,
        start_block,
        attack_block,
        &config.blocks,
        replacee_contract,
    );

    let mut gas_records = HashMap::new();

    let excluded_txs: HashSet<TxHash> =
        if let Some(ref exclude_txs_vec) = config.excluded_transactions {
            exclude_txs_vec
                .iter()
                .map(|tx_hash_str| TxHash::from_str(tx_hash_str).unwrap())
                .collect()
        } else {
            HashSet::new()
        };

    for (tx_hash, gas_price) in txs {
        if excluded_txs.contains(&tx_hash) {
            continue;
        }

        assert!(tx_hash != attack_tx_hash);

        if let Some((gas_usage, eth_price)) =
            replay(bp, config, creation_code, tx_hash)
        {
            gas_records.insert(tx_hash, (gas_usage, gas_price, eth_price));
        }
    }

    gas_records
}

fn get_txs_by_block_range<
    P: ReceiptProvider
        + TransactionsProvider
        + BlockNumReader
        + BlockHashReader
        + StateProviderFactory
        + EvmEnvProvider,
>(
    bp: &P,
    start_block: u64,
    end_block: u64,
    blocks: &Option<Vec<u64>>,
    replacee_contract: Address,
) -> HashMap<TxHash, U256> {
    let mut rv = HashMap::new();

    let blocks: Option<HashSet<u64>> = blocks
        .as_ref()
        .map(|blocks| blocks.iter().cloned().collect());

    let mut finded_blocks = Vec::new();
    for block in start_block..end_block {
        let state = BcStateBuilder::fork_at(bp, block).unwrap();

        if let Some(ref select) = blocks {
            if !select.contains(&block) {
                continue;
            }
        }

        println!("Processing block {}", block);
        let txs = bp.transactions_by_block(block.into()).unwrap();
        let txs = match txs {
            Some(txs) => txs,
            None => continue,
        };
        let spec = TransitionSpecBuilder::new()
            .at_block(bp, block)
            .append_signed_txs(txs.clone())
            .build();

        let mut inspector =
            InternalTransactionInspector::new(replacee_contract);
        _ = BcState::transit(state, spec, &mut inspector).unwrap();

        if !inspector.txs.is_empty() {
            finded_blocks.push(block);
        }

        for i in inspector.txs {
            let tx_hash = txs.get(i).unwrap().hash;
            println!("found tx: {:?}", tx_hash);
            let (tx, tx_meta) =
                bp.transaction_by_hash_with_meta(tx_hash).unwrap().unwrap();
            let gas_price = ToPrimitive::cvt(
                tx.transaction.effective_gas_price(tx_meta.base_fee),
            );
            rv.insert(tx_hash, gas_price);
        }
    }
    println!("{:?}", finded_blocks);

    rv
}

fn get_eth_price<BS: Database<Error = E> + DatabaseCommit, E: fmt::Debug>(
    state: &mut BS,
) -> U256 {
    let caller = HighLevelCaller::from(Address::zero()).bypass_check();
    let chainlink =
        Address::from_str("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419")
            .unwrap();
    let ret = caller
        .call(
            state,
            chainlink,
            &[0xfe, 0xaf, 0x96, 0x8c],
            None,
            no_inspector(),
        )
        .expect("Failed to get ETH price");

    let answer_byte = ret[32..64].to_vec();
    U256::try_from_be_slice(answer_byte.as_slice()).unwrap()
}

fn replay<
    P: ReceiptProvider
        + TransactionsProvider
        + BlockNumReader
        + BlockHashReader
        + StateProviderFactory
        + EvmEnvProvider,
>(
    bp: &P,
    config: &Config,
    creation_code: &str,
    tx_hash: TxHash,
) -> Option<(u64, U256)> {
    let (tx, tx_meta) =
        bp.transaction_by_hash_with_meta(tx_hash).unwrap().unwrap();
    let block = TxPosition::new(tx_meta.block_number, tx_meta.index);

    let mut state = BcStateBuilder::fork_at(bp, block).unwrap();
    let eth_price = get_eth_price(&mut state);

    if let Some(ref addr_str) = config.replacee_address {
        let addr = Address::from_str(addr_str).unwrap();

        let (code_hash, bytecode) = deploy(bp, config, creation_code);

        let mut account_info = state.basic(addr).unwrap().unwrap();
        if account_info.code_hash != code_hash {
            account_info.code_hash = code_hash;
            account_info.code = Some(bytecode);
            state.insert_account_info(addr, account_info);
        }

        if let Some(storages) = &config.storages {
            for (slot, value) in storages {
                let slot = U256::from(*slot);
                let value = ToPrimitive::cvt(value.as_str());
                let _ = state.insert_account_storage(addr, slot, value);
            }
        }

        let mut spec_builder =
            TransitionSpecBuilder::new().at_block(bp, block.block);
        spec_builder = spec_builder.append_signed_tx(tx);
        let mut spec = spec_builder.build();

        // make the sender rich enough to support the additional gas fee
        spec.txs[0].gas_limit += 500_000;
        let mut info = state.basic(spec.txs[0].caller).unwrap().unwrap();
        info.balance += U256::from(10).pow(U256::from(22));
        state.insert_account_info(spec.txs[0].caller, info);

        let (_, results) =
            BcState::transit(state, spec, no_inspector()).unwrap();
        let result = results[0].clone();
        match result {
            ExecutionResult::Success { gas_used, .. } => {
                Some((gas_used, eth_price))
            }
            _ => {
                println!("Replay Failed ({:?}): {:?}", tx_hash, result);
                None
            }
        }
    } else {
        let receipts = bp.receipts_by_block(block.block).unwrap().unwrap();
        if block.index == 0 {
            Some((receipts[0].cumulative_gas_used, eth_price))
        } else {
            Some((
                receipts[block.index as usize].cumulative_gas_used
                    - receipts[(block.index - 1) as usize].cumulative_gas_used,
                eth_price,
            ))
        }
    }
}

fn deploy<
    P: ReceiptProvider
        + TransactionsProvider
        + BlockNumReader
        + BlockHashReader
        + StateProviderFactory
        + EvmEnvProvider,
>(
    bp: &P,
    config: &Config,
    creation_code: &str,
) -> (B256, Bytecode) {
    let block = TxPosition::new(config.block, 0);
    let mut state = BcStateBuilder::fork_at(bp, block).unwrap();

    let deployer = Address::from_str(&config.deployer).unwrap();
    let caller: HighLevelCaller =
        HighLevelCaller::from(deployer).bypass_check();
    let data = hex::decode(creation_code.trim()).unwrap();

    let (_, addr) = caller
        .create(&mut state, &data, None, no_inspector())
        .unwrap();

    let deployed_addr = addr.unwrap();

    let code_hash = state.basic(deployed_addr).unwrap().unwrap().code_hash;
    (code_hash, state.code_by_hash(code_hash).unwrap())
}
