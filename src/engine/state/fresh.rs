#[cfg(test)]
mod tests_nodep {

    use reth_primitives::{Transaction, TransactionKind, TxLegacy};

    use revm::Database;
    use revm_primitives::{
        AccountInfo, Address, BlockEnv, Bytecode, CfgEnv, ExecutionResult, U256,
    };

    use crate::engine::{
        inspectors::no_inspector,
        state::env::TransitionSpecBuilder,
        state::{BcState, BcStateBuilder},
        transactions::Tx,
    };

    #[test]
    fn test_fresh_state_with_plain_transfer() {
        let spender = Address::from(0);
        let receiver = Address::from(1);

        // set cfg and env
        let cfg = CfgEnv {
            disable_block_gas_limit: true,
            disable_base_fee: true,
            ..Default::default()
        };
        let block_env = BlockEnv {
            gas_limit: U256::from(1000000),
            ..Default::default()
        };

        // create state
        let mut state = BcStateBuilder::fresh();
        {
            let acc = AccountInfo::new(
                U256::from(1000),
                Default::default(),
                Bytecode::new(),
            );
            state.insert_account_info(spender, acc);
            let acc = AccountInfo::new(
                U256::from(0),
                Default::default(),
                Bytecode::new(),
            );
            state.insert_account_info(receiver, acc);
        }

        let tx_inner = Transaction::Legacy(TxLegacy {
            to: TransactionKind::Call(receiver),
            value: 500,
            gas_limit: 100000,
            ..Default::default()
        });
        let tx = Tx::Unsigned((spender, tx_inner.clone()));

        // simulate
        let spec = TransitionSpecBuilder::new()
            .set_cfg(cfg.clone())
            .set_block(block_env.clone())
            .append_tx(tx.from(), tx)
            .build();
        let result = BcState::dry_run(&state, spec, no_inspector())
            .unwrap()
            .pop()
            .unwrap();

        assert!(matches!(result, ExecutionResult::Success { .. }));
        let spender_balance = state.basic(spender).unwrap().unwrap().balance;
        assert_eq!(
            spender_balance,
            U256::from(1000),
            "spender balance should be unchanged in simulation"
        );
        let receiver_balance = state.basic(receiver).unwrap().unwrap().balance;
        assert_eq!(
            receiver_balance,
            U256::from(0),
            "receiver balance should be unchanged in simulation"
        );

        // transact
        let tx = Tx::Unsigned((spender, tx_inner));
        let spec = TransitionSpecBuilder::new()
            .set_cfg(cfg)
            .set_block(block_env)
            .append_tx(tx.from(), tx)
            .build();
        let (mut state, mut result) =
            BcState::transit(state, spec, no_inspector()).unwrap();
        let result = result.pop().unwrap();

        assert!(matches!(result, ExecutionResult::Success { .. }));
        let spender_balance = state.basic(spender).unwrap().unwrap().balance;
        assert_eq!(
            spender_balance,
            U256::from(500),
            "spender balance should be decreased by 500"
        );
        let receiver_balance = state.basic(receiver).unwrap().unwrap().balance;
        assert_eq!(
            receiver_balance,
            U256::from(500),
            "receiver balance should be increased by 500"
        );
    }
}
