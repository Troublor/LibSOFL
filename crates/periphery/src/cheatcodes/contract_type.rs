use std::fmt::Debug;

use alloy_dyn_abi::JsonAbiExt;
use alloy_sol_types::{SolCall, SolType};
use libsofl_core::{
    conversion::ConvertTo,
    engine::{
        state::BcState,
        types::{Address, U256},
    },
    error::SoflError,
};

use crate::{
    addressbook::{
        AaveLendingPoolV2ABI, CurveCryptoRegistryABI, CurveRegistryABI,
        UniswapV2FactoryABI, UniswapV2PairABI, UniswapV3FactoryABI,
        UniswapV3PoolABI, ADDRESS_BOOK,
    },
    types::{Chain, SolAddress},
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
    pub fn get_contract_type<S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Result<ContractType, SoflError>
    where
        S::Error: Debug,
        S: BcState,
    {
        if let Some(ty) = self.__get_contract_type(state, address) {
            Ok(ty)
        } else {
            Ok(ContractType::Unknown)
        }
    }

    pub fn __get_contract_type<S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
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
    pub fn __check_curve_y_vault<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        println!("token: {:?}", token);
        let func = self
            .parse_abi("function token() returns (address)")
            .expect("bug: invalid abi");
        let calldata = func.abi_encode_input(&[]).expect("bug: invalid abi");
        let ret = self.cheat_read(state, token, calldata.cvt()).ok()?;
        let token = SolAddress::abi_decode(&ret, true).ok()?;

        println!("token: {:?}", token);

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

    pub fn __check_aave_atoken_v2<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        let func = self
            .parse_abi("underlyingAssetAddress() returns (address)")
            .expect("bug: invalid abi");
        let calldata = func.abi_encode_input(&[]).expect("bug: invalid abi");
        let base_token: Address = if let Ok(unerlying_token) =
            self.cheat_read(state, token, calldata.cvt())
        {
            SolAddress::abi_decode(&unerlying_token, true).ok()?
        } else {
            let func = self
                .parse_abi("UNDERLYING_ASSET_ADDRESS() returns (address)")
                .expect("bug: invalid abi");
            let calldata =
                func.abi_encode_input(&[]).expect("bug: invalid abi");
            let ret = self.cheat_read(state, token, calldata.cvt()).ok()?;
            SolAddress::abi_decode(&ret, true).ok()?
        };

        let call =
            AaveLendingPoolV2ABI::getReserveDataCall { asset: base_token };
        let calldata = call.abi_encode();
        let ret = self
            .cheat_read(
                state,
                ADDRESS_BOOK
                    .aave_lending_pool_v2
                    .must_on_chain(Chain::Mainnet),
                calldata.cvt(),
            )
            .ok()?;
        let rets: AaveLendingPoolV2ABI::ReserveData =
            AaveLendingPoolV2ABI::getReserveDataCall::abi_decode_returns(
                &ret, true,
            )
            .ok()?
            ._0;
        if rets.aTokenAddress == token {
            Some(ContractType::AaveAToken(base_token))
        } else {
            None
        }
    }
}

impl CheatCodes {
    pub fn __check_curve_lp_token<S>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        // check CurveStableSwap
        {
            let call =
                CurveRegistryABI::get_pool_from_lp_tokenCall { arg0: token };
            let calldata = call.abi_encode();
            let ret = self
                .cheat_read(
                    state,
                    ADDRESS_BOOK.curve_registry.must_on_chain(Chain::Mainnet),
                    calldata.cvt(),
                )
                .ok()?;
            let ret = CurveRegistryABI::get_pool_from_lp_tokenCall::abi_decode_returns(&ret, true)
                .ok()?;
            let pool = ret._0;
            if pool != Address::ZERO {
                let contract_type = self.get_contract_type(state, pool).ok()?;
                return Some(ContractType::CurveStableSwapToken(
                    pool,
                    Box::new(contract_type),
                ));
            }
        }

        // check CurveCryptoSwap
        {
            let call = CurveCryptoRegistryABI::get_pool_from_lp_tokenCall {
                arg0: token,
            };
            let calldata = call.abi_encode();
            let ret = self
                .cheat_read(
                    state,
                    ADDRESS_BOOK
                        .curve_crypto_registry
                        .must_on_chain(Chain::Mainnet),
                    calldata.cvt(),
                )
                .ok()?;
            let ret =
                CurveCryptoRegistryABI::get_pool_from_lp_tokenCall::abi_decode_returns(&ret, true)
                    .ok()?;
            let pool = ret._0;
            if pool != Address::ZERO {
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
    fn __check_uniswap_v2<S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        // check token info
        let token0 = {
            let call = UniswapV2PairABI::token0Call {};
            let calldata = call.abi_encode();
            let ret = self.cheat_read(state, address, calldata.cvt()).ok()?;
            UniswapV2PairABI::token0Call::abi_decode_returns(&ret, true)
                .ok()?
                ._0
        };

        let token1 = {
            let call = UniswapV2PairABI::token1Call {};
            let calldata = call.abi_encode();
            let ret = self.cheat_read(state, address, calldata.cvt()).ok()?;
            UniswapV2PairABI::token1Call::abi_decode_returns(&ret, true)
                .ok()?
                ._0
        };

        // check from the perspective of factory
        let call = UniswapV2FactoryABI::getPairCall {
            _0: token0,
            _1: token1,
        };
        let calldata = call.abi_encode();
        let ret = self
            .cheat_read(
                state,
                ADDRESS_BOOK
                    .uniswap_v2_factory
                    .must_on_chain(Chain::Mainnet),
                calldata.cvt(),
            )
            .ok()?;
        let ret =
            UniswapV2FactoryABI::getPairCall::abi_decode_returns(&ret, true)
                .ok()?;
        if ret._0 == address {
            Some(ContractType::UniswapV2Pair(token0, token1))
        } else {
            None
        }
    }

    fn __check_uniswap_v3<S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        // get token and fee info
        let token0 = {
            let call = UniswapV3PoolABI::token0Call {};
            let calldata = call.abi_encode();
            let ret = self.cheat_read(state, address, calldata.cvt()).ok()?;
            UniswapV3PoolABI::token0Call::abi_decode_returns(&ret, true)
                .ok()?
                ._0
        };

        let token1 = {
            let call = UniswapV3PoolABI::token1Call {};
            let calldata = call.abi_encode();
            let ret = self.cheat_read(state, address, calldata.cvt()).ok()?;
            UniswapV3PoolABI::token1Call::abi_decode_returns(&ret, true)
                .ok()?
                ._0
        };

        let fee = {
            let call = UniswapV3PoolABI::feeCall {};
            let calldata = call.abi_encode();
            let ret = self.cheat_read(state, address, calldata.cvt()).ok()?;
            UniswapV3PoolABI::feeCall::abi_decode_returns(&ret, true)
                .ok()?
                ._0
        };

        // check with factory
        let call = UniswapV3FactoryABI::getPoolCall {
            _0: token0,
            _1: token1,
            _2: fee,
        };
        let calldata = call.abi_encode();
        let ret = self
            .cheat_read(
                state,
                ADDRESS_BOOK
                    .uniswap_v3_factory
                    .must_on_chain(Chain::Mainnet),
                calldata.cvt(),
            )
            .ok()?;
        let pool =
            UniswapV3FactoryABI::getPoolCall::abi_decode_returns(&ret, true)
                .ok()?
                ._0;
        if pool == address {
            Some(ContractType::UniswapV3Pool(token0, token1, fee.cvt()))
        } else {
            None
        }
    }

    fn __check_curve_stableswap<S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        let call = CurveRegistryABI::get_coinsCall { _pool: address };
        let calldata = call.abi_encode();
        let ret = self
            .cheat_read(
                state,
                ADDRESS_BOOK.curve_registry.must_on_chain(Chain::Mainnet),
                calldata.cvt(),
            )
            .ok()?;
        let coins =
            CurveRegistryABI::get_coinsCall::abi_decode_returns(&ret, true)
                .ok()?
                ._0;

        if !coins.is_empty() && coins[0] != Address::ZERO {
            return Some(ContractType::CurveStableSwap(
                coins.into_iter().filter(|x| *x != Address::ZERO).collect(),
            ));
        }

        None
    }

    fn __check_curve_cryptoswap<S>(
        &mut self,
        state: &mut S,
        address: Address,
    ) -> Option<ContractType>
    where
        S::Error: Debug,
        S: BcState,
    {
        let call = CurveCryptoRegistryABI::get_coinsCall { _pool: address };
        let calldata = call.abi_encode();
        let ret = self
            .cheat_read(
                state,
                ADDRESS_BOOK
                    .curve_crypto_registry
                    .must_on_chain(Chain::Mainnet),
                calldata.cvt(),
            )
            .ok()?;
        let coins = CurveCryptoRegistryABI::get_coinsCall::abi_decode_returns(
            &ret, true,
        )
        .ok()?
        ._0;

        if !coins.is_empty() && coins[0] != Address::ZERO {
            return Some(ContractType::CurveCryptoSwap(
                coins.into_iter().filter(|x| *x != Address::ZERO).collect(),
            ));
        }

        None
    }
}

#[cfg(test)]
mod tests_with_dep {
    use libsofl_core::{
        blockchain::{provider::BcStateProvider, tx_position::TxPosition},
        conversion::ConvertTo,
        engine::types::Address,
    };

    use crate::{
        cheatcodes::{contract_type::ContractType, CheatCodes},
        test::get_test_bc_provider,
    };

    #[test]
    fn test_get_pegged_token_type() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new(1, 17000001)
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        let curve_yvault: Address =
            "0xE537B5cc158EB71037D4125BDD7538421981E6AA".cvt();
        let token_ty = cheatcodes
            .get_contract_type(&mut state, curve_yvault)
            .unwrap();
        assert!(matches!(token_ty, ContractType::CurveYVault(_)));

        let ausdc: Address = "0xbcca60bb61934080951369a648fb03df4f96263c".cvt();
        let token_ty = cheatcodes.get_contract_type(&mut state, ausdc).unwrap();
        assert!(matches!(token_ty, ContractType::AaveAToken(_)));
    }

    #[test]
    fn test_get_dex_type() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new(1, 17000001);

        let uniswap_v2: Address =
            "0x004375Dff511095CC5A197A54140a24eFEF3A416".cvt();
        let uniswap_v3: Address =
            "0x7668B2Ea8490955F68F5c33E77FE150066c94fb9".cvt();
        let curve_stable_swap: Address =
            "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".cvt();
        let curve_crypto_swap: Address =
            "0x752eBeb79963cf0732E9c0fec72a49FD1DEfAEAC".cvt();
        let random: Address =
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".cvt();

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
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new(1, 17000001)
            .set_caller(&|caller| caller.at_block(&bp, fork_at.block));

        let token = "0x3Ba78eC6Fdd9E1AD64c1a28F5Db6D63156565fF9".cvt();
        let (pool, pool_ty) = cheatcodes
            .get_contract_type(&mut state, token)
            .unwrap()
            .get_pool(token)
            .unwrap();
        let expected: Address =
            "0x3Ba78eC6Fdd9E1AD64c1a28F5Db6D63156565fF9".cvt();
        assert_eq!(pool, expected);
        assert!(matches!(pool_ty, ContractType::UniswapV2Pair(_, _)));

        let token = "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".cvt();
        let token_ty = cheatcodes.get_contract_type(&mut state, token).unwrap();
        let (pool, pool_ty) = token_ty.get_pool(token).unwrap();
        let expected: Address =
            "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".cvt();
        assert_eq!(pool, expected);
        assert!(matches!(pool_ty, ContractType::CurveStableSwap(_)));

        let token = "0xc4AD29ba4B3c580e6D59105FFf484999997675Ff".cvt();
        let (pool, pool_ty) = cheatcodes
            .get_contract_type(&mut state, token)
            .unwrap()
            .get_pool(token)
            .unwrap();
        let expected: Address =
            "0xD51a44d3FaE010294C616388b506AcdA1bfAAE46".cvt();
        assert_eq!(pool, expected);
        assert!(matches!(pool_ty, ContractType::CurveCryptoSwap(_)));
    }
}
