// This module perform differential testing between the InterruptableEvm and the original Evm.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use libsofl_core::{
        blockchain::{
            provider::{BcProvider, BcStateProvider},
            transaction::Tx,
        },
        conversion::ConvertTo,
        engine::{
            interruptable::differential_testing::{
                differential_testing, differential_testing_one_tx,
            },
            memory::MemoryBcState,
            transition::TransitionSpec,
            types::TxHash,
        },
    };
    use libsofl_utils::config::Config;

    use crate::{
        blockchain::{
            provider::RethProvider, state::RethBcStateRef, transaction::RethTx,
        },
        config::RethConfig,
    };

    fn get_bc_provider() -> Arc<RethProvider> {
        let provider = RethConfig::must_load().bc_provider().unwrap();
        Arc::new(provider)
    }

    #[test]
    fn test_replay_17000000() {
        let p = get_bc_provider();
        let report = differential_testing(p, 17000000).unwrap();
        // println!("{:?}", report.unwrap());
        assert!(report.is_none());
    }

    #[test]
    fn test_replay_18000000() {
        let p = get_bc_provider();
        let report = differential_testing(p, 17000000).unwrap();
        // println!("{:?}", report.unwrap());
        assert!(report.is_none());
    }

    #[test]
    fn test_replay_14000000_to_14000100() {
        let p = get_bc_provider();
        for bn in 14000000..14000100 {
            println!("Testing block {}", bn);
            let report = differential_testing(p.clone(), bn).unwrap();
            // println!("{:?}", report.unwrap());
            assert!(report.is_none());
        }
    }

    #[test]
    fn test_replay_tx_0xc0b30971c2774a8e1c274989bb9911838e3bf66e109a2a378e16cf40d814f8d3(
    ) {
        let p = get_bc_provider();
        let tx_hash: TxHash = "0xc0b30971c2774a8e1c274989bb9911838e3bf66e109a2a378e16cf40d814f8d3"
            .parse()
            .unwrap();
        let tx = p.tx(tx_hash.cvt()).unwrap();
        let spec = TransitionSpec::from_tx_hash(&p, tx_hash).unwrap();
        let state = p.bc_state_at(tx.position().unwrap()).unwrap();
        let report = differential_testing_one_tx::<
            RethTx,
            MemoryBcState<RethBcStateRef>,
            Arc<RethProvider>,
        >(state, tx_hash, spec)
        .unwrap();
        // let r = report.unwrap();
        // println!("{:?}", r.output.1);
        assert!(report.is_none());
    }
}
