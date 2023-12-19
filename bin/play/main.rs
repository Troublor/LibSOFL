use std::str::FromStr;

use libsofl::engine::providers::reth::RethBcProvider;
use libsofl::engine::providers::rpc::JsonRpcBcProvider;
use libsofl::engine::providers::BcProviderBuilder;
use libsofl::engine::state::env::TransitionSpec;
use libsofl::engine::state::{BcState, BcStateBuilder};
use libsofl::engine::transactions::position::TxPosition;
use libsofl::engine::{
    cheatcodes::CheatCodes, transactions::builder::TxBuilder,
};
use reth_primitives::{Address, TxHash};

fn main() {
    let bp = BcProviderBuilder::default_db().unwrap();

    let attack_tx_hash = TxHash::from_str(
        "0xaa79afe1a556284a16117bea20bcfad49f4c3ab3a371ba06b5ebdcebc0b6a331",
    )
    .unwrap();
    let victim_tx_hash = TxHash::from_str(
        "0xca04cec2436f9ad5b13345da20d8b2a569bbaa8be2d4e6ba640fe6f0ff4c28e7",
    )
    .unwrap();
    let mut state =
        BcStateBuilder::fork_before_tx(&bp, attack_tx_hash).unwrap();
    let attack_tx = TransitionSpec::from_tx_hash(&bp, attack_tx_hash).unwrap();
    let victim_tx = TransitionSpec::from_tx_hash(&bp, victim_tx_hash).unwrap();
    let (_, mut rs) =
        BcState::transit_without_inspector(state, victim_tx).unwrap();
    let r = rs.remove(0);
    println!("victim tx result: {:?}", r);
}
