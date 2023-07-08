use ethers::abi::{self, Token};
use libafl::{
    prelude::{
        Executor, HasObservers, ObserversTuple, UsesInput, UsesObservers,
    },
    state::UsesState,
};
use reth_primitives::Address;
use reth_provider::{
    EvmEnvProvider, StateProviderFactory, TransactionsProvider,
};
use revm::{Database, DatabaseCommit};
use revm_primitives::{db::DatabaseRef, U256};

use crate::{
    engine::{
        cheatcodes::CheatCodes,
        inspectors::{self, no_inspector},
        state::{
            env::TransitionSpecBuilder, BcState, BcStateBuilder,
            DatabaseEditable, ForkedState, ForkedStateDbError,
        },
        transactions::position::TxPosition,
        utils::HighLevelCaller,
    },
    error::SoflError,
    fuzzing::{
        corpus::tx::TxInput,
        observer::{
            asset_flow::{AssetFlowObserver, DifferentialAssetFlowObserver},
            DifferentialEvmObserverTuple,
        },
    },
    unwrap_token_values,
    utils::{
        abi::{
            CURVE_CRYPTO_POOL_ABI, CURVE_CRYPTO_REGISTRY_ABI, CURVE_POOL_ABI,
            CURVE_REGISTRY_ABI, ERC20_ABI, UNISWAP_V2_FACTORY_ABI,
            UNISWAP_V2_PAIR_ABI,
        },
        addresses::{self},
        conversion::{Convert, ToEthers, ToPrimitive},
        math::{HPMultipler, UFixed256},
    },
};

#[derive(Debug)]
pub struct PomExecutor<S, BS> {
    observers: (DifferentialAssetFlowObserver, ()),
    state: BS,
    spec_builder: TransitionSpecBuilder,

    _phantom: std::marker::PhantomData<S>,
}

impl<'a, S> PomExecutor<S, ForkedState<'a>> {
    pub fn new<
        P: TransactionsProvider + EvmEnvProvider + StateProviderFactory,
    >(
        p: &'a P,
        pos: impl Into<TxPosition>,
    ) -> Result<Self, SoflError<ForkedStateDbError>> {
        let pos: TxPosition = pos.into();
        let state = BcStateBuilder::fork_at(p, pos)?;
        let spec_builder = TransitionSpecBuilder::new()
            .bypass_check()
            .at_block(p, pos.block);
        Ok(Self {
            observers: (DifferentialAssetFlowObserver::default(), ()),
            state,
            spec_builder,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<S: UsesInput<Input = TxInput>, BS> UsesState for PomExecutor<S, BS> {
    type State = S;
}

impl<EM, Z, S, BS> Executor<EM, Z> for PomExecutor<S, BS>
where
    EM: UsesState<State = Self::State>,
    Z: UsesState<State = Self::State>,
    S: UsesInput<Input = TxInput> + std::fmt::Debug,
    BS: std::fmt::Debug + DatabaseRef,
{
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut Self::State,
        _mgr: &mut EM,
        input: &Self::Input,
    ) -> Result<libafl::prelude::ExitKind, libafl::Error> {
        // execute input without price oracle manipulation
        let spec = self
            .spec_builder
            .clone()
            .append_tx(input.from(), input)
            .build();
        let bc_state = BcStateBuilder::fork(&self.state);
        let mut insp = <(
            DifferentialAssetFlowObserver,
            (),
        ) as DifferentialEvmObserverTuple<
            S,
            revm::db::CacheDB<&BS>,
            (AssetFlowObserver, ()),
            (AssetFlowObserver, ()),
        >>::get_first_inspector(
            &mut self.observers, &bc_state, input
        )?;
        let (post_state, result) = BcState::transit(bc_state, spec, &mut insp)
            .map_err(|_| {
                libafl::Error::IllegalArgument(
                    "failed to execute transaction".to_string(),
                    libafl::ErrorBacktrace::new(),
                )
            })?;
        let exit_kind = libafl::prelude::ExitKind::Ok;
        <(
            DifferentialAssetFlowObserver,
            (),
        ) as DifferentialEvmObserverTuple<
            S,
            revm::db::CacheDB<&BS>,
            (AssetFlowObserver, ()),
            (AssetFlowObserver, ()),
        >>::on_first_executed(
            &mut self.observers,
            &post_state,
            insp,
            result,
            input,
        )?;
        self.observers.post_exec_all(state, input, &exit_kind)?;

        // execute input with price oracle manipulation
        // TODO: todo!("manipulate price oracle");
        let spec = self
            .spec_builder
            .clone()
            .append_tx(input.from(), input)
            .build();
        let bc_state = BcStateBuilder::fork(&self.state);
        let mut insp = <(
            DifferentialAssetFlowObserver,
            (),
        ) as DifferentialEvmObserverTuple<
            S,
            revm::db::CacheDB<&BS>,
            (AssetFlowObserver, ()),
            (AssetFlowObserver, ()),
        >>::get_first_inspector(
            &mut self.observers, &bc_state, input
        )?;
        let (post_state, result) = BcState::transit(bc_state, spec, &mut insp)
            .map_err(|_| {
                libafl::Error::IllegalArgument(
                    "failed to execute transaction".to_string(),
                    libafl::ErrorBacktrace::new(),
                )
            })?;
        let exit_kind = libafl::prelude::ExitKind::Ok;
        <(
            DifferentialAssetFlowObserver,
            (),
        ) as DifferentialEvmObserverTuple<
            S,
            revm::db::CacheDB<&BS>,
            (AssetFlowObserver, ()),
            (AssetFlowObserver, ()),
        >>::on_second_executed(
            &mut self.observers,
            &post_state,
            insp,
            result,
            input,
        )?;
        self.observers.post_exec_all(state, input, &exit_kind)?;

        Ok(libafl::prelude::ExitKind::Ok)
    }
}
impl<S: UsesInput<Input = TxInput>, BS> UsesObservers for PomExecutor<S, BS> {
    type Observers = (DifferentialAssetFlowObserver, ());
}

impl<S: UsesInput<Input = TxInput>, BS> HasObservers for PomExecutor<S, BS> {
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}

pub enum Flation {
    Inflation(UFixed256),
    Deflation(UFixed256),
}

#[derive(Debug, Clone, Default)]
pub struct NaivePriceOracleManipulator {
    caller: HighLevelCaller,
}

impl NaivePriceOracleManipulator {
    pub fn new(caller: HighLevelCaller) -> Self {
        Self { caller }
    }
}

impl NaivePriceOracleManipulator {
    #[allow(unused)]
    fn manipulate_uniswap_v2<
        E: std::fmt::Debug,
        BS: Database<Error = E> + DatabaseCommit + DatabaseEditable<Error = E>,
    >(
        &mut self,
        state: &mut BS,
        swap_pool: Address,
        direction: Flation,
    ) -> Result<(), SoflError<E>> {
        let mut cheatcodes = CheatCodes::new();

        // UniswapV2-like AMM
        // get current reserves
        let get_reserves_func = UNISWAP_V2_PAIR_ABI.function("getReserves")?;
        let caller = self.caller.clone();
        let ret = caller.view(
            state,
            swap_pool,
            get_reserves_func,
            &[],
            no_inspector(),
        )?;
        let (reserve0, reserve1) = unwrap_token_values!(ret, Uint, Uint);
        let k = reserve0 * reserve1;

        // calculate new reserves
        let (reserve0, reserve1) = match direction {
            Flation::Inflation(slippage) => {
                let r1: U256 = (HPMultipler::default()
                    * (slippage.denominator() + slippage.raw_value)
                    * reserve1
                    / slippage.denominator())
                .into();
                let r0: U256 = (HPMultipler::default()
                    * reserve0
                    * slippage.denominator()
                    / (slippage.denominator() + slippage.raw_value))
                    .into();
                (r0, r1)
            }
            Flation::Deflation(slippage) => {
                let r1: U256 = (HPMultipler::default()
                    * (slippage.denominator() - slippage.raw_value)
                    * reserve1
                    / slippage.denominator())
                .into();
                let r0: U256 = (HPMultipler::default()
                    * reserve0
                    * slippage.denominator()
                    / (slippage.denominator() - slippage.raw_value))
                    .into();
                (r0, r1)
            }
        };

        // get token contracts
        let token0_func = UNISWAP_V2_PAIR_ABI.function("token0")?;
        let token1_func = UNISWAP_V2_PAIR_ABI.function("token1")?;
        let ret =
            caller.view(state, swap_pool, token0_func, &[], no_inspector())?;
        let (token0,) = unwrap_token_values!(ret, Address);
        let ret =
            caller.view(state, swap_pool, token1_func, &[], no_inspector())?;
        let (token1,) = unwrap_token_values!(ret, Address);

        // cheat: set pool token balance to new reserves
        cheatcodes.set_erc20_balance(state, token0, swap_pool, reserve0)?;
        cheatcodes.set_erc20_balance(state, token1, swap_pool, reserve1)?;

        // sync pool
        let sync_func = UNISWAP_V2_PAIR_ABI
            .function("sync")
            .expect("impossible: sync is not a function");
        caller.invoke(
            state,
            swap_pool,
            sync_func,
            &[],
            None,
            no_inspector(),
        )?;

        Ok(())
    }

    ///
    /// inflation: swap the first token by % of its reserve.
    /// deflation: swap the second token by % of its reserve.
    pub fn manipulate_curve_pool<
        E: std::fmt::Debug,
        BS: Database<Error = E> + DatabaseCommit + DatabaseEditable<Error = E>,
    >(
        &mut self,
        state: &mut BS,
        pool: Address,
        is_crypto_pool: bool,
        pair: (Address, Address),
        flation: Flation,
    ) -> Result<(), SoflError<E>> {
        let caller = self.caller.clone();
        let mut cheatcodes = CheatCodes::new();
        // manipulate curve plain pool price by performing an exchange
        println!("before manipulate");
        // mainipulate pool
        let registry: Address;
        let registry_abi: &ethers::abi::Contract;
        let pool_abi: &ethers::abi::Contract;
        let idx0: U256;
        let idx1: U256;
        let is_underlying: bool;
        if is_crypto_pool {
            registry = *addresses::CURVE_CRYPTO_REGISTRY;
            registry_abi = &CURVE_CRYPTO_REGISTRY_ABI;
            pool_abi = &CURVE_CRYPTO_POOL_ABI;
            (idx0, idx1, is_underlying) = unwrap_token_values!(
                caller.view(
                    state,
                    registry,
                    registry_abi.function("get_coin_indices").expect(
                        "impossible: get_coin_indices function does not exist"
                    ),
                    &[
                        ToEthers::cvt(pool),
                        ToEthers::cvt(pair.0),
                        ToEthers::cvt(pair.1),
                    ],
                    no_inspector()
                )?,
                Uint,
                Uint,
                bool
            );
        } else {
            registry = *addresses::CURVE_REGISTRY;
            registry_abi = &CURVE_REGISTRY_ABI;
            pool_abi = &CURVE_POOL_ABI;
            let i0;
            let i1;
            (i0, i1, is_underlying) = unwrap_token_values!(
                caller.view(
                    state,
                    registry,
                    registry_abi.function("get_coin_indices").expect(
                        "impossible: get_coin_indices function does not exist"
                    ),
                    &[
                        ToEthers::cvt(pool),
                        ToEthers::cvt(pair.0),
                        ToEthers::cvt(pair.1),
                    ],
                    no_inspector()
                )?,
                Int,
                Int,
                bool
            );
            idx0 = ToPrimitive::cvt(
                ethers::types::U256::try_from(i0)
                    .expect("impossible: i0 is not a U256"),
            );
            idx1 = ToPrimitive::cvt(
                ethers::types::U256::try_from(i1)
                    .expect("impossible: i1 is not a U256"),
            );
        }
        println!("before flation");
        let (token_in, _token_out, idx_in, idx_out, amount_in) = match flation {
            Flation::Inflation(percent) => {
                let (in_reserve,) = unwrap_token_values!(
                    caller.view(
                        state,
                        pool,
                        pool_abi.function("balances").expect(
                            "impossible: balances function does not exist"
                        ),
                        &[ToEthers::cvt(idx0)],
                        no_inspector()
                    )?,
                    Uint
                );
                let amount_in: U256 = (HPMultipler::default()
                    * in_reserve
                    * (percent.denominator() + percent.raw_value)
                    / percent.denominator())
                .into();
                (pair.0, pair.1, idx0, idx1, amount_in)
            }
            Flation::Deflation(percent) => {
                let (in_reserve,) = unwrap_token_values!(
                    caller.view(
                        state,
                        pool,
                        pool_abi.function("balances").expect(
                            "impossible: balances function does not exist"
                        ),
                        &[ToEthers::cvt(idx1)],
                        no_inspector()
                    )?,
                    Uint
                );
                let amount_in: U256 = (HPMultipler::default()
                    * in_reserve
                    * (percent.denominator() + percent.raw_value)
                    / percent.denominator())
                .into();
                (pair.1, pair.0, idx1, idx0, amount_in)
            }
        };
        let value;
        if token_in == *addresses::ETH {
            value = Some(amount_in);
        } else {
            value = None;
            // faucet tokens for caller
            cheatcodes.set_erc20_balance(
                state,
                token_in,
                caller.address,
                amount_in,
            )?;
            // approve
            caller.invoke(
                state,
                token_in,
                ERC20_ABI
                    .function("approve")
                    .expect("impossible: approve function does not exist"),
                &[ToEthers::cvt(pool), ToEthers::cvt(amount_in)],
                None,
                no_inspector(),
            )?;
        }
        let mut pool_args: Vec<Token> = Vec::new();
        if is_crypto_pool {
            pool_args.push(ToEthers::cvt(idx_in));
            pool_args.push(ToEthers::cvt(idx_out));
            pool_args.push(ToEthers::cvt(amount_in));
            pool_args.push(ToEthers::cvt(0));
        } else {
            pool_args.push(Token::Int(ToEthers::cvt(idx_in)));
            pool_args.push(Token::Int(ToEthers::cvt(idx_out)));
            pool_args.push(ToEthers::cvt(amount_in));
            pool_args.push(ToEthers::cvt(0));
        }
        println!("where");
        // exchange
        if is_underlying {
            caller.invoke(
                state,
                pool,
                pool_abi.function("exchange_underlying").expect(
                    "impossible: exchange_underlying function does not exist",
                ),
                &pool_args,
                value,
                no_inspector(),
            )?;
        } else {
            caller.invoke(
                state,
                pool,
                pool_abi
                    .function("exchange")
                    .expect("impossible: exchange function does not exist"),
                &pool_args,
                value,
                no_inspector(),
            )?;
        }
        Ok(())
    }
}

pub fn get_uniswap_v2_pair_address<
    E: std::fmt::Debug,
    BS: Database<Error = E> + DatabaseCommit,
>(
    state: &mut BS,
    token0: Address,
    token1: Address,
) -> Result<Address, SoflError<E>> {
    let caller = HighLevelCaller::default().bypass_check();
    let get_pair_func = UNISWAP_V2_FACTORY_ABI
        .function("getPair")
        .expect("impossible: getPair is not a function");
    let pair = caller
        .view(
            state,
            *addresses::UNISWAP_V2_FACTORY,
            get_pair_func,
            &[
                abi::Token::Address(ToEthers::cvt(token0)),
                abi::Token::Address(ToEthers::cvt(token1)),
            ],
            inspectors::no_inspector(),
        )
        .unwrap();
    // todo!()
    let (p,) = unwrap_token_values!(pair, Address);
    Ok(p)
}

pub fn get_uniswap_v2_reserves<
    E: std::fmt::Debug,
    BS: Database<Error = E> + DatabaseCommit,
>(
    state: &mut BS,
    pair: Address,
) -> Result<(U256, U256), SoflError<E>> {
    let caller = HighLevelCaller::default().bypass_check();
    let get_reserves_func = UNISWAP_V2_PAIR_ABI
        .function("getReserves")
        .expect("impossible: getReserves is not a function");
    let ret = caller
        .view(
            state,
            pair,
            get_reserves_func,
            &[],
            inspectors::no_inspector(),
        )
        .unwrap();
    let (reserve0, reserve1) = unwrap_token_values!(ret, Uint, Uint);
    Ok((reserve0, reserve1))
}

#[cfg(test)]
mod tests_with_jsonrpc {

    use std::str::FromStr;

    use ethers::abi::Token;
    use reth_primitives::Address;

    use revm_primitives::U256;

    use crate::{
        engine::{
            inspectors::no_inspector, providers::rpc::JsonRpcBcProvider,
            state::BcStateBuilder, utils::HighLevelCaller,
        },
        unwrap_token_values,
        utils::{
            abi::CURVE_POOL_ABI,
            addresses,
            conversion::{Convert, ToEthers, ToPrimitive},
            math::UFixed256,
        },
    };

    use super::{
        get_uniswap_v2_pair_address, get_uniswap_v2_reserves, Flation,
    };

    #[test]
    fn test_manipulate_uniswap_v2_eth_usdc_price() {
        let provider = JsonRpcBcProvider::default();
        let mut state = BcStateBuilder::fork_at(&provider, 16000000).unwrap();
        let pair = get_uniswap_v2_pair_address(
            &mut state,
            *addresses::WETH,
            *addresses::USDC,
        )
        .unwrap();
        let (_r0, r1) = get_uniswap_v2_reserves(&mut state, pair).unwrap();
        let mut manipulator = super::NaivePriceOracleManipulator::new(
            HighLevelCaller::new(ToPrimitive::cvt(1))
                .bypass_check()
                .at_block(&provider, 16000000),
        );
        manipulator
            .manipulate_uniswap_v2(
                &mut state,
                pair,
                super::Flation::Inflation(UFixed256 {
                    raw_value: U256::from(1),
                    decimals: 1,
                }),
            )
            .unwrap();
        let (_r0_, r1_) = get_uniswap_v2_reserves(&mut state, pair).unwrap();
        assert_eq!(r1 * U256::from(3) / U256::from(2), r1_);
        // assert_eq!(r0 * r1, r0_ * r1_);
    }

    fn manipulate_curve(
        pool: Address,
        pool_abi: &ethers::abi::Contract,
        pair: (Address, Address),
        is_crypto_pool: bool,
    ) {
        let provider = JsonRpcBcProvider::default();
        let mut state = BcStateBuilder::fork_at(&provider, 14972421).unwrap();
        let caller = HighLevelCaller::new(ToPrimitive::cvt(1))
            .bypass_check()
            .at_block(&provider, 14972421);
        let usdc_idx: u128 = 1;
        let usdt_idx: u128 = 2;
        let test_amount_in = U256::from(1_000_000u128);
        let (amount_out_before,) = unwrap_token_values!(
            caller
                .view(
                    &mut state,
                    pool,
                    pool_abi.function("get_dy").unwrap(),
                    &[
                        if is_crypto_pool {
                            ToEthers::cvt(usdc_idx)
                        } else {
                            Token::Int(ToEthers::cvt(usdc_idx))
                        },
                        if is_crypto_pool {
                            ToEthers::cvt(usdt_idx)
                        } else {
                            Token::Int(ToEthers::cvt(usdt_idx))
                        },
                        ToEthers::cvt(test_amount_in),
                    ],
                    no_inspector(),
                )
                .unwrap(),
            Uint
        );
        let mut manipulator =
            super::NaivePriceOracleManipulator::new(caller.clone());
        manipulator
            .manipulate_curve_pool(
                &mut state,
                pool,
                is_crypto_pool,
                pair,
                Flation::Inflation(UFixed256 {
                    raw_value: U256::from(1),
                    decimals: 1,
                }),
            )
            .unwrap();
        let (amount_out_after,) = unwrap_token_values!(
            caller
                .view(
                    &mut state,
                    pool,
                    pool_abi.function("get_dy").unwrap(),
                    &[
                        if is_crypto_pool {
                            ToEthers::cvt(usdc_idx)
                        } else {
                            Token::Int(ToEthers::cvt(usdc_idx))
                        },
                        if is_crypto_pool {
                            ToEthers::cvt(usdt_idx)
                        } else {
                            Token::Int(ToEthers::cvt(usdt_idx))
                        },
                        ToEthers::cvt(test_amount_in),
                    ],
                    no_inspector()
                )
                .unwrap(),
            Uint
        );
        println!("{:?} => {:?}", amount_out_before, amount_out_after);
        assert!(amount_out_before > amount_out_after);
    }

    macro_rules! gen_manipulate_curve_test {
        ($name:ident, $pool:expr, $abi:expr, $pair: expr, $is_crypto:expr) => {
            #[test]
            fn $name() {
                manipulate_curve($pool, $abi, $pair, $is_crypto);
            }
        };
    }

    // gen_manipulate_curve_test!(
    //     test_manipulate_curve_usdc_usdt,
    //     Address::from_str("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")
    //         .unwrap(),
    //     &CURVE_POOL_ABI,
    //     (*addresses::USDC, *addresses::USDT),
    //     false
    // );
    gen_manipulate_curve_test!(
        test_manipulate_curve_usdc_usdt_lending,
        Address::from_str("0xDeBF20617708857ebe4F679508E7b7863a8A8EeE")
            .unwrap(),
        &CURVE_POOL_ABI,
        (*addresses::USDC, *addresses::USDT),
        false
    );
    // gen_manipulate_curve_test!(
    //     test_manipulate_curve_usdt_wbtc,
    //     Address::from_str("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
    //         .unwrap(),
    //     &CURVE_CRYPTO_POOL_ABI,
    //     (*addresses::USDT, *addresses::WBTC),
    //     true
    // );
}
