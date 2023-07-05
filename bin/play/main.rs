use std::path::Path;
use std::str::FromStr;

use libsofl::config::flags::SoflConfig;
use libsofl::engine::cheatcodes::{CheatCodes, ERC20Cheat, PriceOracleCheat};
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

    {
        let usdc =
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                .unwrap();
        {
            let balance = cheatcode
                .get_erc20_balance(&mut state, usdc, account)
                .unwrap();
            println!("balance: {} {} : {}", usdc, account, balance);

            let total_supply =
                cheatcode.get_erc20_total_supply(&mut state, usdc).unwrap();
            println!("total supply: {} : {}", usdc, total_supply);

            let decimals =
                cheatcode.get_erc20_decimals(&mut state, usdc).unwrap();
            println!("decimals: {} : {}", usdc, decimals);
        }

        cheatcode
            .set_erc20_balance(&mut state, usdc, account, U256::from(1234567))
            .unwrap();
        {
            let balance = cheatcode
                .get_erc20_balance(&mut state, usdc, account)
                .unwrap();
            println!("balance: {} {} : {}", usdc, account, balance);

            let total_supply =
                cheatcode.get_erc20_total_supply(&mut state, usdc).unwrap();
            println!("total supply: {} : {}", usdc, total_supply);

            let decimals =
                cheatcode.get_erc20_decimals(&mut state, usdc).unwrap();
            println!("decimals: {} : {}", usdc, decimals);
        }
    }

    {
        let wbtc =
            Address::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599")
                .unwrap();
        let price = cheatcode.get_price_in_ether(&mut state, wbtc).unwrap();
        let decimals = cheatcode.get_erc20_decimals(&mut state, wbtc).unwrap();
        println!("price: {} : {} : {}", wbtc, price, decimals);
    }
    {
        let weth =
            Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                .unwrap();
        let price = cheatcode.get_price_in_ether(&mut state, weth).unwrap();
        let decimals = cheatcode.get_erc20_decimals(&mut state, weth).unwrap();
        println!("price: {} : {} : {}", weth, price, decimals);
    }
    {
        let usdc: Address =
            Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .unwrap();
        let price = cheatcode.get_price_in_ether(&mut state, usdc).unwrap();
        let decimals = cheatcode.get_erc20_decimals(&mut state, usdc).unwrap();
        println!("price: {} : {} : {}", usdc, price, decimals);
    }
    {
        let dai: Address =
            Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
                .unwrap();
        let price = cheatcode.get_price_in_ether(&mut state, dai).unwrap();
        let decimals = cheatcode.get_erc20_decimals(&mut state, dai).unwrap();
        println!("price: {} : {} : {}", dai, price, decimals);
    }
    {
        let usdt: Address =
            Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7")
                .unwrap();
        let price = cheatcode.get_price_in_ether(&mut state, usdt).unwrap();
        let decimals = cheatcode.get_erc20_decimals(&mut state, usdt).unwrap();
        println!("price: {} : {} : {}", usdt, price, decimals);
    }
}
