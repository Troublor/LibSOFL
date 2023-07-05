use ethers::abi;
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
use revm_primitives::{
    db::DatabaseRef, hex::ToHex, ruint::aliases::U512, U256,
};

use crate::{
    engine::{
        cheatcodes::{CheatCodes, ERC20Cheat},
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
        abi::{UNISWAP_V2_FACTORY_ABI, UNISWAP_V2_PAIR_ABI},
        addresses,
        conversion::{Convert, ToEthers, ToPrimitive},
        math::UFixed256,
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

pub enum SlippageDirection {
    Up(UFixed256),
    Down(UFixed256),
}

#[derive(Debug, Clone, Default)]
pub struct NaivePriceOracleManipulator {}

impl NaivePriceOracleManipulator {
    fn manipulate<
        E: std::fmt::Debug,
        BS: Database<Error = E> + DatabaseCommit + DatabaseEditable<Error = E>,
    >(
        &mut self,
        state: &mut BS,
        swap_pool: Address,
        direction: SlippageDirection,
    ) -> Result<(), SoflError<E>> {
        // UniswapV2-like AMM
        // get current reserves
        let get_reserves_func = UNISWAP_V2_PAIR_ABI.function("getReserves")?;
        let caller = HighLevelCaller::default().bypass_check();
        let mut et = caller.view(
            state,
            swap_pool,
            get_reserves_func,
            &[],
            no_inspector(),
        )?;
        let (reserve0, reserve1) = unwrap_token_values!(ret, Uint, Uint);
        let reserve0 = reserve0.to::<U512>();
        let reserve1 = reserve1.to::<U512>();
        let k = reserve0 * reserve1;

        // calculate new reserves
        let reserve1 = match direction {
            SlippageDirection::Up(slippage) => {
                let changes1 = reserve1 * slippage.raw_value.to::<U512>()
                    / slippage.denominator().to::<U512>();
                reserve1 + changes1
            }
            SlippageDirection::Down(slippage) => {
                let changes1 = reserve1 * slippage.raw_value.to::<U512>()
                    / slippage.denominator().to::<U512>();
                reserve1 - changes1
            }
        };
        let reserve0 = U256::wrapping_from(k / reserve1);
        let reserve1 = U256::wrapping_from(reserve1);

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
        let mut cheatcode = CheatCodes::new();
        println!("token0: {:?}", token0);
        println!("token1: {:?}", token1);
        cheatcode.set_erc20_balance(state, token0, swap_pool, reserve0)?;
        cheatcode.set_erc20_balance(state, token1, swap_pool, reserve1)?;

        // sync pool
        let sync_func = UNISWAP_V2_PAIR_ABI
            .function("sync")
            .expect("impossible: sync is not a function");
        caller.view(state, swap_pool, sync_func, &[], no_inspector())?;

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
    let mut ret = caller
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
    use ethers::abi::{self, Token};
    use revm_primitives::U256;

    use crate::{
        engine::{
            inspectors::{self, no_inspector},
            providers::rpc::JsonRpcBcProvider,
            state::BcStateBuilder,
            utils::HighLevelCaller,
        },
        utils::{
            abi::UNISWAP_V2_FACTORY_ABI,
            addresses,
            conversion::{Convert, ToEthers},
            math::UFixed256,
        },
    };

    use super::{get_uniswap_v2_pair_address, get_uniswap_v2_reserves};

    #[test]
    fn test_manipulate_eth_usdc_price() {
        let provider = JsonRpcBcProvider::default();
        let mut state = BcStateBuilder::fork_at(&provider, 16000000).unwrap();
        let pair = get_uniswap_v2_pair_address(
            &mut state,
            *addresses::WETH,
            *addresses::USDC,
        )
        .unwrap();
        println!("pair: {:?}", pair);
        let (r0, r1) = get_uniswap_v2_reserves(&mut state, pair).unwrap();
        println!(
            "before manipulation => r0: {:?}, r1: {:?}, k: {:?}",
            r0,
            r1,
            r0 * r1
        );
        let mut manipulator = super::NaivePriceOracleManipulator::default();
        manipulator
            .manipulate(
                &mut state,
                pair,
                super::SlippageDirection::Up(UFixed256 {
                    raw_value: U256::from(1),
                    decimals: 1,
                }),
            )
            .unwrap();
        let (r0, r1) = get_uniswap_v2_reserves(&mut state, pair).unwrap();
        println!(
            "after manipulation => r0: {:?}, r1: {:?}, k: {:?}",
            r0,
            r1,
            r0 * r1,
        );
    }
}
