#[cfg(test)]
mod tests_with_db {
    use std::path::Path;

    use reth_provider::{ReceiptProvider, TransactionsProvider};
    use revm_primitives::ExecutionResult;

    use crate::{
        config::flags::SoflConfig,
        engine::{
            inspectors::no_inspector,
            providers::BcProviderBuilder,
            state::{env::TransitionSpecBuilder, BcState, BcStateBuilder},
            transactions::position::TxPosition,
        },
    };

    #[test]
    fn test_reproduce_block() {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();
        let fork_at = TxPosition::new(17000000, 0);
        let txs = bp.transactions_by_block(fork_at.block).unwrap().unwrap();
        let receipts = bp.receipts_by_block(fork_at.block).unwrap().unwrap();

        // prepare state
        let state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        // prepare cfg and env
        let spec = TransitionSpecBuilder::new()
            .at_block(&bp, fork_at.block)
            .append_signed_txs(txs)
            .build();

        // execute
        let (_, results) =
            BcState::transit(state, spec, no_inspector()).unwrap();

        assert_eq!(results.len(), receipts.len());

        for (result, receipt) in results.iter().zip(receipts.iter()) {
            match result {
                ExecutionResult::Success { logs, .. } => {
                    assert!(receipt.success);
                    assert_eq!(receipt.logs.len(), logs.len());
                    for (log, receipt_log) in
                        logs.iter().zip(receipt.logs.iter())
                    {
                        assert_eq!(log.address, receipt_log.address);
                        assert_eq!(log.topics, receipt_log.topics);
                        assert_eq!(*log.data, *receipt_log.data);
                    }
                }
                _ => assert!(!receipt.success),
            }
        }
    }
}

#[cfg(test)]
mod tests_with_dep {
    use reth_provider::ReceiptProvider;

    use crate::{
        engine::{
            inspectors::no_inspector,
            state::{env::TransitionSpec, BcState, BcStateBuilder},
            transactions::position::TxPosition,
        },
        utils::{
            conversion::{Convert, ToPrimitive},
            testing::get_testing_bc_provider,
        },
    };

    #[test]
    fn test_reproduce_tx() {
        let bp = get_testing_bc_provider();
        let fork_at = TxPosition::new(17000000, 0);

        // prepare state
        let state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        // collect the tx
        let tx_hash = ToPrimitive::cvt("0xa278205118a242c87943b9ed83aacafe9906002627612ac3672d8ea224e38181");
        let spec = TransitionSpec::from_tx_hash(&bp, tx_hash).unwrap();

        // simulate
        let r = BcState::dry_run(&state, spec, no_inspector())
            .unwrap()
            .pop()
            .unwrap();
        assert!(r.is_success());
        let receipt = bp.receipt_by_hash(tx_hash).unwrap().unwrap();
        assert_eq!(receipt.success, r.is_success());
        assert_eq!(receipt.logs.len(), r.logs().len());
        for (log, receipt_log) in r.logs().iter().zip(receipt.logs.iter()) {
            assert_eq!(log.address, receipt_log.address);
            assert_eq!(log.topics, receipt_log.topics);
            assert_eq!(*log.data, *receipt_log.data);
        }
    }
}
