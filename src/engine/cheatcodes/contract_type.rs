use revm::{Database, DatabaseCommit};
use revm_primitives::Address;
use std::fmt::Debug;

use crate::{
    engine::state::DatabaseEditable,
    error::SoflError,
    unwrap_first_token_value,
    utils::{
        abi::{CURVE_POOL_ABI, UNISWAP_V2_PAIR_ABI, UNISWAP_V3_POOL_ABI},
        addresses::{CURVE_POOL_OWNER, UNISWAP_V2_FACTORY, UNISWAP_V3_FACTORY},
    },
};

use super::CheatCodes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractType {
    UniswapV2Pair,
    UniswapV3Pool,
    CurveStableSwap,
}

impl CheatCodes {
    pub fn get_contract_type<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Result<Option<ContractType>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        {
            let func = UNISWAP_V2_PAIR_ABI.function("factory").expect(
                "bug: cannot find factory function in UniswapV2Pair ABI",
            );

            if let Ok(mut tokens) = self.cheat_read(state, address, func, &[]) {
                if unwrap_first_token_value!(Address, tokens)
                    == *UNISWAP_V2_FACTORY
                {
                    return Ok(Some(ContractType::UniswapV2Pair));
                }
            }
        }

        {
            let func = UNISWAP_V3_POOL_ABI.function("factory").expect(
                "bug: cannot find factory function in UniswapV3Pool ABI",
            );

            if let Ok(mut tokens) = self.cheat_read(state, address, func, &[]) {
                if unwrap_first_token_value!(Address, tokens)
                    == *UNISWAP_V3_FACTORY
                {
                    return Ok(Some(ContractType::UniswapV3Pool));
                }
            }
        }

        {
            let func = CURVE_POOL_ABI.function("owner").expect(
                "bug: cannot find owner function in CurveStableSwap ABI",
            );
            if let Ok(mut tokens) = self.cheat_read(state, address, func, &[]) {
                if unwrap_first_token_value!(Address, tokens)
                    == *CURVE_POOL_OWNER
                {
                    return Ok(Some(ContractType::CurveStableSwap));
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests_with_jsonrpc {
    use std::str::FromStr;

    use reth_primitives::Address;

    use crate::engine::cheatcodes::{CheatCodes, ContractType};
    use crate::engine::providers::rpc::JsonRpcBcProvider;
    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;

    #[test]
    fn test_match_contract_type() {
        let bp = JsonRpcBcProvider::default();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

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
            cheatcodes
                .get_contract_type(&mut state, uniswap_v2)
                .unwrap(),
            Some(ContractType::UniswapV2Pair)
        );

        assert_eq!(
            cheatcodes
                .get_contract_type(&mut state, uniswap_v3)
                .unwrap(),
            Some(ContractType::UniswapV3Pool)
        );

        assert_eq!(
            cheatcodes
                .get_contract_type(&mut state, curve_stable_swap)
                .unwrap(),
            Some(ContractType::CurveStableSwap)
        );

        assert!(cheatcodes
            .get_contract_type(&mut state, random)
            .unwrap()
            .is_none());
    }
}
