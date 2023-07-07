use revm::{Database, DatabaseCommit};
use revm_primitives::Address;
use std::fmt::Debug;

use crate::{
    engine::state::DatabaseEditable,
    error::SoflError,
    unwrap_first_token_value,
    utils::{abi::UNISWAP_V2_PAIR_ABI, addresses::UNISWAP_V2_FACTORY},
};

use super::{global_cheatcodes_unsafe, CheatCodes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractType {
    UniswapV2Pair,
    UniswapV3Pool,
    CurveStableSwap,
}

impl CheatCodes {
    pub fn get_contract_type<E, S>(
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

            if let Ok(mut tokens) =
                global_cheatcodes_unsafe().cheat_read(state, address, func, &[])
            {
                if unwrap_first_token_value!(Address, tokens)
                    == *UNISWAP_V2_FACTORY
                {
                    return Ok(Some(ContractType::UniswapV2Pair));
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
            CheatCodes::get_contract_type(&mut state, uniswap_v2).unwrap(),
            Some(ContractType::UniswapV2Pair)
        );

        assert!(CheatCodes::get_contract_type(&mut state, random)
            .unwrap()
            .is_none());
    }
}
