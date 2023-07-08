use std::str::FromStr;

use libsofl::engine::cheatcodes::{CheatCodes, ContractType};
use libsofl::engine::providers::rpc::JsonRpcBcProvider;
use libsofl::engine::state::BcStateBuilder;
use libsofl::engine::transactions::position::TxPosition;
use reth_primitives::Address;

fn main() {
    let bp = JsonRpcBcProvider::default();

    let fork_at = TxPosition::new(17000001, 0);
    let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

    let uniswap_v2 =
        Address::from_str("0x004375Dff511095CC5A197A54140a24eFEF3A416")
            .unwrap();
    let uniswap_v3 =
        Address::from_str("0x7668B2Ea8490955F68F5c33E77FE150066c94fb9")
            .unwrap();
    let curve_stable_swap =
        Address::from_str("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")
            .unwrap();
    let random =
        Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .unwrap();

    assert_eq!(
        CheatCodes::get_contract_type(&mut state, uniswap_v2).unwrap(),
        Some(ContractType::UniswapV2Pair)
    );
    println!("2");

    assert_eq!(
        CheatCodes::get_contract_type(&mut state, uniswap_v3).unwrap(),
        Some(ContractType::UniswapV3Pool)
    );
    println!("3");
    assert_eq!(
        CheatCodes::get_contract_type(&mut state, curve_stable_swap).unwrap(),
        Some(ContractType::CurveStableSwap)
    );
    println!("4");

    assert!(CheatCodes::get_contract_type(&mut state, random)
        .unwrap()
        .is_none());
    println!("5");
}
