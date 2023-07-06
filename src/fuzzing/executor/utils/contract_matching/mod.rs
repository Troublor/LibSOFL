use reth_primitives::Address;
use revm::Database;
use revm_primitives::Bytes;
use std::fmt::Debug;

mod uniswap_v2;
use uniswap_v2::UNISWAP_V2_PAIR_CODE;

mod uniswap_v3;
use uniswap_v3::UNISWAP_V3_POOL_CODE;

use crate::{
    engine::{cheatcodes::CheatCodes, state::DatabaseEditable},
    error::SoflError,
};

const MATCHING_THERSHOLD: f64 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractType {
    UniswapV2,
    UniswapV3,
}

fn match_score(code: Bytes, template: &[u8]) -> f64 {
    let mut score = 0.0;

    for (i, byte) in code.iter().enumerate() {
        if byte == &template[i] {
            score += 1.0;
        }
    }

    score / code.len() as f64
}

pub fn match_contract_type<E, S>(
    state: &mut S,
    account: Address,
    threshold: Option<f64>,
) -> Result<Option<ContractType>, SoflError<<S as Database>::Error>>
where
    E: Debug,
    S: DatabaseEditable<Error = E> + Database<Error = E>,
{
    let code = CheatCodes::get_code(state, account)?;
    let threshold = threshold.unwrap_or(MATCHING_THERSHOLD);

    println!("{} {}", code.len(), UNISWAP_V2_PAIR_CODE.len());

    if code.len() == UNISWAP_V2_PAIR_CODE.len()
        && match_score(code.original_bytes(), UNISWAP_V2_PAIR_CODE) > threshold
    {
        return Ok(Some(ContractType::UniswapV2));
    }

    if code.len() == UNISWAP_V3_POOL_CODE.len()
        && match_score(code.original_bytes(), UNISWAP_V3_POOL_CODE) > threshold
    {
        return Ok(Some(ContractType::UniswapV3));
    }

    Ok(None)
}

#[cfg(test)]
mod tests_with_db {
    use std::{path::Path, str::FromStr};

    use reth_primitives::Address;

    use crate::engine::state::BcStateBuilder;
    use crate::fuzzing::executor::utils::contract_matching::{
        match_contract_type, ContractType,
    };
    use crate::{
        config::flags::SoflConfig,
        engine::{
            providers::BcProviderBuilder, transactions::position::TxPosition,
        },
    };

    #[test]
    fn test_match_contract_type() {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let uniswap_v2 =
            Address::from_str("0x004375Dff511095CC5A197A54140a24eFEF3A416")
                .unwrap();
        let uniswap_v3 =
            Address::from_str("0x7668B2Ea8490955F68F5c33E77FE150066c94fb9")
                .unwrap();
        let random =
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .unwrap();

        assert_eq!(
            match_contract_type(&mut state, uniswap_v2, None).unwrap(),
            Some(ContractType::UniswapV2)
        );

        assert_eq!(
            match_contract_type(&mut state, uniswap_v3, None).unwrap(),
            Some(ContractType::UniswapV3)
        );

        assert_eq!(
            match_contract_type(&mut state, random, None).unwrap(),
            None,
        );
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use std::str::FromStr;

    use reth_primitives::Address;

    use crate::engine::providers::rpc::JsonRpcBcProvider;
    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;
    use crate::fuzzing::executor::utils::contract_matching::{
        match_contract_type, ContractType,
    };

    #[test]
    fn test_match_contract_type() {
        let bp = JsonRpcBcProvider::default();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let uniswap_v2 =
            Address::from_str("0x004375Dff511095CC5A197A54140a24eFEF3A416")
                .unwrap();
        let uniswap_v3 =
            Address::from_str("0x7668B2Ea8490955F68F5c33E77FE150066c94fb9")
                .unwrap();
        let random =
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .unwrap();

        assert_eq!(
            match_contract_type(&mut state, uniswap_v2, None).unwrap(),
            Some(ContractType::UniswapV2)
        );

        assert_eq!(
            match_contract_type(&mut state, uniswap_v3, None).unwrap(),
            Some(ContractType::UniswapV3)
        );

        assert_eq!(
            match_contract_type(&mut state, random, None).unwrap(),
            None,
        );
    }
}
