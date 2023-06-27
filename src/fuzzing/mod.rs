use crate::engine::state::BcState;

pub mod corpus;
pub mod executor;
pub mod feedback;
pub mod generator;
pub mod mutator;
pub mod observer;

#[cfg(test)]
mod tests_others {
    use libafl::{
        prelude::{
            current_nanos, havoc_mutations, tuple_list, BytesInsertMutator,
            Generator, SimpleEventManager, SimpleMonitor, StdRand,
            StdScheduledMutator,
        },
        schedulers::QueueScheduler,
        stages::StdMutationalStage,
        state::StdState,
        Evaluator, Fuzzer, StdFuzzer,
    };
    use reth_primitives::Address;
    use reth_provider::EvmEnvProvider;
    use revm_primitives::{BlockEnv, CfgEnv};

    use crate::{
        engine::{
            providers::rpc::JsonRpcBcProvider, state::fork::ForkedBcState,
            transactions::position::TxPosition,
        },
        utils::conversion::{Convert, ToPrimitive},
    };

    use super::{
        corpus::tx::TxCorpus,
        executor::tx::TxExecutor,
        feedback::{always::AlwaysFeedback, assert::AssertionFeedback},
        generator::history_tx::HistoricalTxGenerator,
        observer::result::ExecutionResultObserver,
    };
    #[test]
    fn test_simple_replay_fuzz() {
        let provider = JsonRpcBcProvider::default();
        let fork_at = TxPosition::new(14000000, 0);
        let contract: Address =
            ToPrimitive::cvt("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let observer = ExecutionResultObserver::default();
        let mut feedback = AlwaysFeedback::default();
        let mut objective = AssertionFeedback::new();
        let mut state = StdState::new(
            StdRand::with_seed(current_nanos()),
            TxCorpus::new(),
            TxCorpus::new(),
            // States of the feedbacks.
            // The feedbacks can report the data that should persist in the State.
            &mut feedback,
            // Same for objective feedbacks
            &mut objective,
        )
        .unwrap();
        let mut cfg = CfgEnv::default();
        let mut block = BlockEnv::default();
        provider
            .fill_env_at(&mut cfg, &mut block, fork_at.block)
            .unwrap();
        let bc_state =
            ForkedBcState::fork_at(&provider, fork_at.clone()).unwrap();
        let mut executor = TxExecutor::new(
            cfg,
            block,
            bc_state,
            tuple_list!(observer),
            &state,
        );
        let scheduler = QueueScheduler::new();
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);
        let mut generator = HistoricalTxGenerator::new(
            &provider,
            contract,
            fork_at.get_block_number(&provider).unwrap(),
        );

        let mut stages = tuple_list!();
        let mon = SimpleMonitor::new(|s| println!("{s}"));
        let mut mgr = SimpleEventManager::new(mon);

        state
            .generate_initial_inputs(
                &mut fuzzer,
                &mut executor,
                &mut generator,
                &mut mgr,
                2,
            )
            .expect("Failed to generate the initial corpus");
        fuzzer
            .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
            .expect("Error in the fuzzing loop");
    }
}
