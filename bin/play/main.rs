use std::str::FromStr;

use libsofl::engine::cheatcodes::CheatCodes;
use libsofl::engine::providers::rpc::JsonRpcBcProvider;
use libsofl::engine::state::BcStateBuilder;
use libsofl::engine::transactions::position::TxPosition;
use reth_primitives::Address;

fn main() {
    let bp = JsonRpcBcProvider::default();

    let fork_at = TxPosition::new(17000001, 0);
    let mut _state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

    let mut _cheatcodes = CheatCodes::new();

    let _uniswap_v2 =
        Address::from_str("0x004375Dff511095CC5A197A54140a24eFEF3A416")
            .unwrap();
    let _uniswap_v3 =
        Address::from_str("0x7668B2Ea8490955F68F5c33E77FE150066c94fb9")
            .unwrap();
    let _curve_stable_swap =
        Address::from_str("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")
            .unwrap();
    let _random =
        Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .unwrap();
    println!("5");
}
