use std::path::Path;
use std::str::FromStr;

use libsofl::config::flags::SoflConfig;
use libsofl::engine::cheatcodes::{CheatCodes, ERC20Cheat};
use libsofl::engine::providers::BcProviderBuilder;
use libsofl::engine::state::BcStateBuilder;
use libsofl::engine::transactions::position::TxPosition;
use reth_primitives::Address;
use revm_primitives::U256;

fn main() {
    let datadir = SoflConfig::load().unwrap().reth.datadir;
    let datadir = Path::new(&datadir);
    let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

    let fork_at = TxPosition::new(17395698, 0);
    let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

    let mut cheatcode = CheatCodes::new();

    let account =
        Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
            .unwrap();

    {
        let weth =
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .unwrap();
        {
            let balance = cheatcode
                .get_erc20_balance(&mut state, weth, account)
                .unwrap();
            println!("balance: {} {} : {}", weth, account, balance);

            let total_supply =
                cheatcode.get_erc20_total_supply(&mut state, weth).unwrap();
            println!("total supply: {} : {}", weth, total_supply);

            let decimals =
                cheatcode.get_erc20_decimals(&mut state, weth).unwrap();
            println!("decimals: {} : {}", weth, decimals);
        }

        cheatcode
            .set_erc20_balance(&mut state, weth, account, U256::from(1234567))
            .unwrap();
        {
            let balance = cheatcode
                .get_erc20_balance(&mut state, weth, account)
                .unwrap();
            println!("balance: {} {} : {}", weth, account, balance);

            let total_supply =
                cheatcode.get_erc20_total_supply(&mut state, weth).unwrap();
            println!("total supply: {} : {}", weth, total_supply);

            let decimals =
                cheatcode.get_erc20_decimals(&mut state, weth).unwrap();
            println!("decimals: {} : {}", weth, decimals);
        }
    }
}
