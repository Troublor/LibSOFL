use std::path::Path;
use std::str::FromStr;

use libsofl::config::flags::SoflConfig;
use libsofl::engine::cheatcodes::CheatCodes;
use libsofl::engine::providers::BcProviderBuilder;
use libsofl::engine::state::BcStateBuilder;
use libsofl::engine::transactions::position::TxPosition;
use reth_primitives::Address;

fn main() {
    let datadir = SoflConfig::load().unwrap().reth.datadir;
    let datadir = Path::new(&datadir);
    let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

    let fork_at = TxPosition::new(17395698, 0);
    let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

    {
        let address1 =
            Address::from_str("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")
                .unwrap();

        let address2 =
            Address::from_str("0xDeBF20617708857ebe4F679508E7b7863a8A8EeE")
                .unwrap();

        let code1 = CheatCodes::get_code(&mut state, address1).unwrap();
        let code2 = CheatCodes::get_code(&mut state, address2).unwrap();

        println!("{} {}", code1.len(), code2.len());
    }
}
