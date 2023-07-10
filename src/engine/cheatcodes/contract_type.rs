use ethers::abi::Token;
use revm::{Database, DatabaseCommit};
use revm_primitives::Address;
use std::fmt::Debug;

use crate::{
    engine::state::DatabaseEditable,
    error::SoflError,
    unwrap_first_token_value,
    utils::{
        abi::{
            CURVE_CRYPTO_REGISTRY_ABI, CURVE_REGISTRY_ABI,
            UNISWAP_V2_FACTORY_ABI, UNISWAP_V2_PAIR_ABI,
            UNISWAP_V3_FACTORY_ABI, UNISWAP_V3_POOL_ABI,
        },
        addresses::{
            CURVE_CRYPTO_REGISTRY, CURVE_REGISTRY, UNISWAP_V2_FACTORY,
            UNISWAP_V3_FACTORY,
        },
    },
};

use super::CheatCodes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractType {
    UniswapV2Pair,
    UniswapV3Pool,
    CurveStableSwap,
    CurveCryptoSwap,
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
        if let Some(ty) = self.__check_uniswap_v2(state, address) {
            return Ok(Some(ty));
        }

        if let Some(ty) = self.__check_uniswap_v3(state, address) {
            return Ok(Some(ty));
        }

        if let Some(ty) = self.__check_curve_stableswap(state, address) {
            return Ok(Some(ty));
        }

        if let Some(ty) = self.__check_curve_cryptoswap(state, address) {
            return Ok(Some(ty));
        }

        Ok(None)
    }

    fn __check_uniswap_v2<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // check token info
        let token0 = {
            let func = UNISWAP_V2_PAIR_ABI.function("token0").expect(
                "bug: cannot find token0 function in UniswapV2Pair ABI",
            );
            unwrap_first_token_value!(
                Address,
                self.cheat_read(state, address, func, &[]).ok()?
            )
        };

        let token1 = {
            let func = UNISWAP_V2_PAIR_ABI.function("token1").expect(
                "bug: cannot find token1 function in UniswapV2Pair ABI",
            );
            unwrap_first_token_value!(
                Address,
                self.cheat_read(state, address, func, &[]).ok()?
            )
        };

        // check from the perspective of factory
        let func = UNISWAP_V2_FACTORY_ABI.function("getPair").expect(
            "bug: cannot find getPair function in UniswapV2Factory ABI",
        );
        if unwrap_first_token_value!(
            Address,
            self.cheat_read(
                state,
                *UNISWAP_V2_FACTORY,
                func,
                &[Token::Address(token0.into()), Token::Address(token1.into())]
            )
            .ok()?
        ) == address
        {
            Some(ContractType::UniswapV2Pair)
        } else {
            None
        }
    }

    fn __check_uniswap_v3<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // get token and fee info
        let token0 = {
            let func = UNISWAP_V3_POOL_ABI.function("token0").expect(
                "bug: cannot find token0 function in UniswapV3Pool ABI",
            );
            unwrap_first_token_value!(
                Address,
                self.cheat_read(state, address, func, &[]).ok()?
            )
        };

        let token1 = {
            let func = UNISWAP_V3_POOL_ABI.function("token1").expect(
                "bug: cannot find token1 function in UniswapV3Pool ABI",
            );
            unwrap_first_token_value!(
                Address,
                self.cheat_read(state, address, func, &[]).ok()?
            )
        };

        let fee = {
            let func = UNISWAP_V3_POOL_ABI
                .function("fee")
                .expect("bug: cannot find fee function in UniswapV3Pool ABI");
            unwrap_first_token_value!(
                Uint,
                self.cheat_read(state, address, func, &[]).ok()?
            )
        };

        // check with factory
        let func = UNISWAP_V3_FACTORY_ABI.function("getPool").expect(
            "bug: cannot find getPool function in UniswapV3Factory ABI",
        );
        if unwrap_first_token_value!(
            Address,
            self.cheat_read(
                state,
                *UNISWAP_V3_FACTORY,
                func,
                &[
                    Token::Address(token0.into()),
                    Token::Address(token1.into()),
                    Token::Uint(fee.into())
                ]
            )
            .ok()?
        ) == address
        {
            Some(ContractType::UniswapV3Pool)
        } else {
            None
        }
    }

    fn __check_curve_stableswap<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let func = CURVE_REGISTRY_ABI
            .function("get_coins")
            .expect("bug: cannot find get_coins function in CurveRegistry ABI");

        let mut tokens = self
            .cheat_read(
                state,
                *CURVE_REGISTRY,
                func,
                &[Token::Address(address.into())],
            )
            .ok()?;
        let coins = unwrap_first_token_value!(Vec<Address>, tokens);
        if !coins.is_empty() && coins[0] != Address::zero() {
            return Some(ContractType::CurveStableSwap);
        }

        None
    }

    fn __check_curve_cryptoswap<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let func = CURVE_CRYPTO_REGISTRY_ABI
            .function("get_coins")
            .expect("bug: cannot find get_coins function in CurveRegistry ABI");

        let mut tokens = self
            .cheat_read(
                state,
                *CURVE_CRYPTO_REGISTRY,
                func,
                &[Token::Address(address.into())],
            )
            .ok()?;
        let coins = unwrap_first_token_value!(Vec<Address>, tokens);
        if !coins.is_empty() && coins[0] != Address::zero() {
            return Some(ContractType::CurveCryptoSwap);
        }

        None
    }
}

#[cfg(test)]
mod tests_with_dep {
    use std::str::FromStr;

    use reth_primitives::Address;

    use crate::engine::cheatcodes::{CheatCodes, ContractType};
    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;
    use crate::utils::testing::get_testing_bc_provider;

    #[test]
    fn test_match_contract_type() {
        let bp = get_testing_bc_provider();

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
        let curve_crypto_swap =
            Address::from_str("0x752eBeb79963cf0732E9c0fec72a49FD1DEfAEAC")
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

        assert_eq!(
            cheatcodes
                .get_contract_type(&mut state, curve_crypto_swap)
                .unwrap(),
            Some(ContractType::CurveCryptoSwap)
        );

        assert!(cheatcodes
            .get_contract_type(&mut state, random)
            .unwrap()
            .is_none());
    }
}
