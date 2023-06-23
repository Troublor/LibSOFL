use std::env;
use std::path::Path;

use libsofl::config::flags::SoflConfig;
use libsofl::engine::providers::BcProviderBuilder;
use libsofl::utils::conversion::{Convert, ToPrimitive};
use reth_provider::TransactionsProvider;
use revm_primitives::B256;

fn main() {
    // get argv
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <txhash>", args[0]);
        return;
    }

    let datadir = SoflConfig::load().unwrap().reth.datadir;
    let datadir = Path::new(&datadir);
    let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

    // get txhash
    let tx_hash: B256 = ToPrimitive::cvt(args[1].as_str());
    let (_, tx_meta) =
        bp.transaction_by_hash_with_meta(tx_hash).unwrap().unwrap();
    println!("{}", tx_meta.block_number);
}
