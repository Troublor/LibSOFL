use ethers::abi::Token;
use revm::{Database, DatabaseCommit};
use revm_primitives::{Address, U256};
use std::fmt::Debug;

use crate::{
    engine::state::DatabaseEditable,
    error::SoflError,
    unwrap_first_token_value,
    utils::{
        abi::{
            AAVE_ATOKEN_V2_ABI, CURVE_CRYPTO_REGISTRY_ABI, CURVE_REGISTRY_ABI,
            UNISWAP_V2_FACTORY_ABI, UNISWAP_V2_PAIR_ABI,
            UNISWAP_V3_FACTORY_ABI, UNISWAP_V3_POOL_ABI,
        },
        addresses::{
            CURVE_CRYPTO_REGISTRY, CURVE_REGISTRY, UNISWAP_V2_FACTORY,
            UNISWAP_V3_FACTORY,
        },
        conversion::{Convert, ToEthers},
    },
};

use super::CheatCodes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractType {
    Unknown,

    // DEX
    UniswapV2Pair(
        Address, // token0
        Address, // token1
    ),
    UniswapV3Pool(
        Address, // token0
        Address, // token1
        U256,    // fee
    ),
    CurveStableSwap(Vec<Address>),
    CurveCryptoSwap(Vec<Address>),

    // LP Token
    CurveStableSwapToken(Address, Box<Self>),
    CurveCryptoSwapToken(Address, Box<Self>),

    // Pegged Token
    CurveYVault(Address),
    AaveAToken(Address),
}

impl ContractType {
    pub fn is_unknown(&self) -> bool {
        matches!(self, ContractType::Unknown)
    }

    pub fn is_dex(&self) -> bool {
        matches!(
            self,
            ContractType::UniswapV2Pair(_, _)
                | ContractType::UniswapV3Pool(_, _, _)
                | ContractType::CurveStableSwap(_)
                | ContractType::CurveCryptoSwap(_)
        )
    }

    pub fn is_lp_token(&self) -> bool {
        matches!(
            self,
            ContractType::UniswapV2Pair(_, _)
                | ContractType::CurveStableSwapToken(_, _)
                | ContractType::CurveCryptoSwapToken(_, _)
        )
    }

    pub fn get_pool(self, token: Address) -> Option<(Address, Self)> {
        match self {
            ContractType::UniswapV2Pair(_, _) => Some((token, self)),
            ContractType::CurveStableSwapToken(pool, pool_ty)
            | ContractType::CurveCryptoSwapToken(pool, pool_ty) => {
                Some((pool, *pool_ty))
            }
            _ => None,
        }
    }

    pub fn is_pegged_token(&self) -> bool {
        matches!(
            self,
            ContractType::CurveYVault(_) | ContractType::AaveAToken(_)
        )
    }

    pub fn get_pegged_token(self, _: Address) -> Option<Address> {
        match self {
            ContractType::CurveYVault(token)
            | ContractType::AaveAToken(token) => Some(token),
            _ => None,
        }
    }
}

macro_rules! check_and_return {
    ($s:expr) => {
        if let Some(ty) = $s {
            return Some(ty);
        }
    };
}

impl CheatCodes {
    pub fn get_contract_type<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Result<ContractType, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        if let Some(ty) = self.__get_contract_type(state, address) {
            Ok(ty)
        } else {
            Ok(ContractType::Unknown)
        }
    }

    pub fn __get_contract_type<E, S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // dex
        check_and_return!(self.__check_uniswap_v2(state, address));
        check_and_return!(self.__check_uniswap_v3(state, address));
        check_and_return!(self.__check_curve_stableswap(state, address));
        check_and_return!(self.__check_curve_cryptoswap(state, address));

        // lp token
        check_and_return!(self.__check_curve_lp_token(state, address));

        // pegged token
        check_and_return!(self.__check_curve_y_vault(state, address));
        check_and_return!(self.__check_aave_atoken_v2(state, address));

        None
    }
}

impl CheatCodes {
    pub fn __check_curve_y_vault<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let func = self
            .parse_abi::<E>("function token() returns (address)".to_string())
            .ok()?
            .clone();
        let token = unwrap_first_token_value!(
            Address,
            self.cheat_read(state, token, &func, &[]).ok()?
        );

        let token_ty = self.get_contract_type(state, token).ok()?;
        if matches!(
            token_ty,
            ContractType::CurveCryptoSwapToken(_, _)
                | ContractType::CurveStableSwapToken(_, _)
        ) {
            Some(ContractType::CurveYVault(token))
        } else {
            None
        }
    }

    pub fn __check_aave_atoken_v2<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let func = AAVE_ATOKEN_V2_ABI
            .function("underlyingAssetAddress")
            .expect("underlyingAssetAddress function not found");

        let token = unwrap_first_token_value!(
            Address,
            self.cheat_read(state, token, func, &[]).ok()?
        );

        println!("underlying token: {}", token);

        None
    }
}

impl CheatCodes {
    pub fn __check_curve_lp_token<E, S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Option<ContractType>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // check CurveStableSwap
        {
            let func = CURVE_REGISTRY_ABI
                .function("get_pool_from_lp_token")
                .unwrap();
            let pool = unwrap_first_token_value!(
                Address,
                self.cheat_read(
                    state,
                    *CURVE_REGISTRY,
                    func,
                    &[ToEthers::cvt(token)]
                )
                .ok()?
            );
            if pool != Address::zero() {
                let contract_type = self.get_contract_type(state, pool).ok()?;
                return Some(ContractType::CurveStableSwapToken(
                    pool,
                    Box::new(contract_type),
                ));
            }
        }

        // check CurveCryptoSwap
        {
            let func = CURVE_CRYPTO_REGISTRY_ABI
                .function("get_pool_from_lp_token")
                .unwrap();
            let pool = unwrap_first_token_value!(
                Address,
                self.cheat_read(
                    state,
                    *CURVE_CRYPTO_REGISTRY,
                    func,
                    &[ToEthers::cvt(token)]
                )
                .ok()?
            );
            if pool != Address::zero() {
                let contract_type = self.get_contract_type(state, pool).ok()?;
                return Some(ContractType::CurveCryptoSwapToken(
                    pool,
                    Box::new(contract_type),
                ));
            }
        }

        None
    }
}

impl CheatCodes {
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
            Some(ContractType::UniswapV2Pair(token0, token1))
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
            Some(ContractType::UniswapV3Pool(token0, token1, fee))
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
            return Some(ContractType::CurveStableSwap(
                coins
                    .into_iter()
                    .filter(|x| *x != Address::zero())
                    .collect(),
            ));
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
            return Some(ContractType::CurveCryptoSwap(
                coins
                    .into_iter()
                    .filter(|x| *x != Address::zero())
                    .collect(),
            ));
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
    use crate::utils::conversion::{Convert, ToPrimitive};
    use crate::utils::testing::get_testing_bc_provider;

    #[test]
    fn test_get_pegged_token_type() {
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new()
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        let curve_yvault =
            ToPrimitive::cvt("0xE537B5cc158EB71037D4125BDD7538421981E6AA");
        let token_ty = cheatcodes
            .get_contract_type(&mut state, curve_yvault)
            .unwrap();
        assert!(matches!(token_ty, ContractType::CurveYVault(_)));

        let ausdc =
            ToPrimitive::cvt("0x9bA00D6856a4eDF4665BcA2C2309936572473B7E");
        let token_type =
            cheatcodes.get_contract_type(&mut state, ausdc).unwrap();
        // assert!(matches!(token_ty, ContractType::AaveAToken(_)));
    }

    #[test]
    fn test_get_dex_type() {
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

        assert!(matches!(
            cheatcodes
                .get_contract_type(&mut state, uniswap_v2)
                .unwrap(),
            ContractType::UniswapV2Pair(_, _)
        ));

        assert!(matches!(
            cheatcodes
                .get_contract_type(&mut state, uniswap_v3)
                .unwrap(),
            ContractType::UniswapV3Pool(_, _, _)
        ));

        assert!(matches!(
            cheatcodes
                .get_contract_type(&mut state, curve_stable_swap)
                .unwrap(),
            ContractType::CurveStableSwap(_)
        ));

        assert!(matches!(
            cheatcodes
                .get_contract_type(&mut state, curve_crypto_swap)
                .unwrap(),
            ContractType::CurveCryptoSwap(_)
        ));

        assert_eq!(
            cheatcodes.get_contract_type(&mut state, random).unwrap(),
            ContractType::Unknown
        );
    }

    #[test]
    fn test_get_dex_lp_token_type() {
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::default()
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        let token =
            ToPrimitive::cvt("0x3Ba78eC6Fdd9E1AD64c1a28F5Db6D63156565fF9");
        let (pool, pool_ty) = cheatcodes
            .get_contract_type(&mut state, token)
            .unwrap()
            .get_pool(token)
            .unwrap();
        assert_eq!(
            pool,
            ToPrimitive::cvt("0x3Ba78eC6Fdd9E1AD64c1a28F5Db6D63156565fF9")
        );
        assert!(matches!(pool_ty, ContractType::UniswapV2Pair(_, _)));

        let token =
            ToPrimitive::cvt("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490");
        let token_ty = cheatcodes.get_contract_type(&mut state, token).unwrap();
        let (pool, pool_ty) = token_ty.get_pool(token).unwrap();
        assert_eq!(
            pool,
            ToPrimitive::cvt("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")
        );
        assert!(matches!(pool_ty, ContractType::CurveStableSwap(_)));

        let token =
            ToPrimitive::cvt("0xc4AD29ba4B3c580e6D59105FFf484999997675Ff");
        let (pool, pool_ty) = cheatcodes
            .get_contract_type(&mut state, token)
            .unwrap()
            .get_pool(token)
            .unwrap();
        assert_eq!(
            pool,
            ToPrimitive::cvt("0xD51a44d3FaE010294C616388b506AcdA1bfAAE46")
        );
        assert!(matches!(pool_ty, ContractType::CurveCryptoSwap(_)));
    }
}
