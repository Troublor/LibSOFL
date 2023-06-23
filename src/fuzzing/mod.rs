use crate::engine::state::BcState;

pub mod corpus;
pub mod executor;
pub mod feedback;
pub mod generator;
pub mod mutator;
pub mod observer;

#[cfg(test)]
mod tests_nodep {
    use libafl::{
        prelude::{current_nanos, StdRand},
        state::StdState,
    };
    use reth_primitives::Address;
    use revm_primitives::CfgEnv;

    use crate::utils::conversion::{Convert, ToPrimitive};

    use super::{
        corpus::tx::TxCorpus, executor::tx::TxExecutor,
        feedback::assert::AssertionFeedback,
        observer::result::ExecutionResultObserver,
    };

    #[test]
    fn test_simple_replay_fuzz() {
        let contract: Address =
            ToPrimitive::cvt("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let observer = ExecutionResultObserver::new();
        let mut feedback = AssertionFeedback::new();
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
        let cfg = CfgEnv {
            ..Default::default()
        };
        // let executor = TxExecutor::new();
    }
}
