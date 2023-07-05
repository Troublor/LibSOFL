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

    {
        let udsc_v2 =
            Address::from_str("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")
                .unwrap();
        let wbtc_v2 =
            Address::from_str("0x004375Dff511095CC5A197A54140a24eFEF3A416")
                .unwrap();

        let usdc_code = CheatCodes::get_code(&mut state, udsc_v2).unwrap();
        let wbtc_code = CheatCodes::get_code(&mut state, wbtc_v2).unwrap();

        println!("{} {}", usdc_code.bytecode.len(), wbtc_code.bytecode.len());
        println!("{} {}", usdc_code.len(), wbtc_code.len());

        // let mut equal_amount = 0;
        // for i in 0..usdc_code.len() {
        //     if usdc_code.bytecode[i] == wbtc_code.bytecode[i] {
        //         equal_amount += 1;
        //     }
        // }

        let equal_amount = usdc_code
            .original_bytes()
            .iter()
            .zip(wbtc_code.original_bytes().iter())
            .map(
                |(usdc_byte, wbtc_byte)| {
                    if usdc_byte == wbtc_byte {
                        1
                    } else {
                        0
                    }
                },
            )
            .sum::<u64>();

        println!(
            "FUCK {} = {} / {}",
            (equal_amount as f64) / (wbtc_code.len() as f64),
            equal_amount,
            wbtc_code.len()
        );
    }

    {
        let dai_v3 =
            Address::from_str("0x60594a405d53811d3bc4766596efd80fd545a270")
                .unwrap();
        let usdt_v3 =
            Address::from_str("0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36")
                .unwrap();

        let dai_code = CheatCodes::get_code(&mut state, dai_v3).unwrap();
        let usdt_code = CheatCodes::get_code(&mut state, usdt_v3).unwrap();

        let equal_amount = dai_code
            .bytecode
            .iter()
            .zip(usdt_code.bytecode.iter())
            .fold(
                0u64,
                |acc, (dai_byte, usdt_byte)| {
                    if dai_byte == usdt_byte {
                        acc + 1
                    } else {
                        acc
                    }
                },
            );

        println!("FUCK {}", (equal_amount as f64) / (usdt_code.len() as f64));

        println!("{:?}", dai_code.original_bytes());
        panic!();
    }

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
