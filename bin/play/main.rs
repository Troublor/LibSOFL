use std::path::Path;
use std::str::FromStr;

use libsofl::config::flags::SoflConfig;
use libsofl::engine::cheatcodes::CheatCodes;
use libsofl::engine::providers::BcProviderBuilder;
use libsofl::engine::state::fork::ForkedBcState;
use libsofl::engine::transactions::position::TxPosition;
use reth_primitives::Address;
use revm_primitives::U256;

fn main() {
    let datadir = SoflConfig::load().unwrap().reth.datadir;
    let datadir = Path::new(&datadir);
    let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

    let fork_at = TxPosition::new(17000001, 0);
    let mut state = ForkedBcState::fork_at(&bp, fork_at).unwrap();

    let mut cheatcode = CheatCodes::default();

    let token = Address::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7")
        .unwrap();
    let account =
        Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
            .unwrap();

    {
        let balance = cheatcode
            .get_erc20_balance(&mut state, token, account)
            .unwrap();
        println!("balance: {} {} : {}", token, account, balance);

        let total_supply =
            cheatcode.get_erc20_total_supply(&mut state, token).unwrap();
        println!("total supply: {} : {}", token, total_supply);

        let decimals = cheatcode.get_erc20_decimals(&mut state, token).unwrap();
        println!("decimals: {} : {}", token, decimals);
    }

    cheatcode
        .set_erc20_balance(&mut state, token, account, U256::from(1234567))
        .unwrap();
    {
        let balance = cheatcode
            .get_erc20_balance(&mut state, token, account)
            .unwrap();
        println!("balance: {} {} : {}", token, account, balance);

        let total_supply =
            cheatcode.get_erc20_total_supply(&mut state, token).unwrap();
        println!("total supply: {} : {}", token, total_supply);

        let decimals = cheatcode.get_erc20_decimals(&mut state, token).unwrap();
        println!("decimals: {} : {}", token, decimals);
    }
}
