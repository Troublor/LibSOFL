use reth_primitives::Address;
use reth_provider::{
    EvmEnvProvider, StateProviderBox, StateProviderFactory,
    TransactionsProvider,
};
use reth_revm::database::State as WrappedDB;
use revm::db::EmptyDB;
use revm::DatabaseCommit;
use revm::{db::CacheDB, Database, EVM};
use revm_primitives::db::DatabaseRef;
use revm_primitives::{AccountInfo, ExecutionResult};
use revm_primitives::{Halt, U256};

use crate::engine::inspectors::no_inspector;
use crate::engine::transactions::position::TxPosition;
use crate::{engine::inspectors::MultiTxInspector, error::SoflError};

use super::env::{TransitionSpec, TransitionSpecBuilder};
use super::DatabaseEditable;

pub type StateProviderDB<'a> = WrappedDB<StateProviderBox<'a>>;

pub struct BcStateBuilder;

impl BcStateBuilder {
    /// Create a forked state from the the state before the transaction at the position is executed.
    pub fn fork_at<
        P: StateProviderFactory + EvmEnvProvider + TransactionsProvider,
    >(
        p: &P,
        pos: impl Into<TxPosition>,
    ) -> Result<CacheDB<StateProviderDB<'_>>, SoflError<reth_interfaces::Error>>
    {
        let pos = pos.into();
        let bn = pos.get_block_number(p).map_err(|_| SoflError::Fork(pos))?;
        let sp = p
            .state_by_block_id((bn - 1).into())
            .map_err(SoflError::Reth)?;
        let wrapped = WrappedDB::new(sp);
        let mut state = CacheDB::new(wrapped);

        // execute proceeding transactions
        if pos.index > 0 {
            let txs = p
                .transactions_by_block(pos.block)
                .map_err(SoflError::Reth)?
                .ok_or(SoflError::Fork(pos))?;
            // prepare
            let mut spec_builder =
                TransitionSpecBuilder::new().at_block(p, pos.block);
            for tx in txs.into_iter().take(pos.index as usize) {
                spec_builder = spec_builder.append_signed_tx(tx);
            }
            let spec = spec_builder.build();
            state = BcState::transit(state, spec, no_inspector())?.0;
        }
        Ok(state)
    }

    /// Create a forked state from the the state after the transaction at the position is executed.
    pub fn fork_from<
        P: StateProviderFactory + EvmEnvProvider + TransactionsProvider,
    >(
        p: &P,
        pos: impl Into<TxPosition>,
    ) -> Result<CacheDB<StateProviderDB<'_>>, SoflError<reth_interfaces::Error>>
    {
        let mut pos = pos.into();
        pos.shift(p, 1).map_err(|_| SoflError::Fork(pos))?;
        Self::fork_at(p, pos)
    }

    /// fork from the current latest blockchain state
    pub fn latest<P: StateProviderFactory + EvmEnvProvider>(
        p: &P,
    ) -> Result<CacheDB<StateProviderDB<'_>>, SoflError<reth_interfaces::Error>>
    {
        let sp = p.latest().map_err(SoflError::Reth)?;
        let wrapped = WrappedDB::new(sp);
        let state = CacheDB::new(wrapped);
        Ok(state)
    }

    pub fn fresh() -> CacheDB<EmptyDB> {
        CacheDB::new(EmptyDB::default())
    }

    pub fn fork<DB: DatabaseRef>(state: DB) -> CacheDB<DB> {
        CacheDB::new(state)
    }
}

pub struct BcState;

impl BcState {
    pub fn modify<
        E,
        BS: Database<Error = E> + DatabaseCommit + DatabaseEditable<Error = E>,
    >(
        mut state: BS,
        func: impl FnOnce(&mut BS) -> Result<(), SoflError<E>>,
    ) -> Result<BS, SoflError<E>> {
        func(&mut state)?;
        Ok(state)
    }

    pub fn transit<BS, I, T>(
        state: BS,
        spec: T,
        mut inspector: &mut I,
    ) -> Result<(BS, Vec<ExecutionResult>), SoflError<BS::Error>>
    where
        BS: Database + DatabaseCommit,
        T: Into<TransitionSpec>,
        I: MultiTxInspector<BS>,
    {
        let TransitionSpec { cfg, block, txs } = spec.into();
        let mut evm = EVM::new();
        evm.env.cfg = cfg;
        evm.env.block = block;
        evm.database(state);
        let mut results = Vec::new();
        for tx in txs.into_iter() {
            let insp = &mut inspector;
            evm.env.tx = tx;
            // inspector pre-transaction hook
            if !insp.transaction(&evm.env.tx, evm.db.as_ref().expect("impossible: db does not exists while database has been set in evm")) {
            // return false to skip transaction
            results.push(ExecutionResult::Halt { reason: Halt::NotActivated, gas_used: 0 });
            continue;
        }
            let result = evm.inspect_commit(insp).map_err(SoflError::Evm)?;
            // inspector post-transaction hook
            (&mut inspector).transaction_end(
            &evm.env.tx,
            evm.db.as_ref().expect("impossible: db does not exists while database has been set in evm"),
            &result,
        );
            results.push(result);
        }
        Ok((
        evm.db.expect(
            "impossible: db does not exists while database has been set in evm",
        ),
        results,
    ))
    }
    pub fn dry_run<'a, E, BS, I, T>(
        state: &'a BS,
        spec: T,
        mut inspector: &mut I,
    ) -> Result<Vec<ExecutionResult>, SoflError<E>>
    where
        BS: DatabaseRef<Error = E> + Database<Error = E> + DatabaseCommit,
        T: Into<TransitionSpec>,
        I: MultiTxInspector<CacheDB<&'a BS>>,
    {
        let state = BcStateBuilder::fork(state);
        let (_, results) = BcState::transit(state, spec, inspector)?;
        Ok(results)
    }
}

impl<DB: DatabaseRef> DatabaseEditable for CacheDB<DB> {
    type Error = DB::Error;

    fn insert_account_storage(
        &mut self,
        address: Address,
        slot: U256,
        value: U256,
    ) -> Result<(), Self::Error> {
        self.insert_account_storage(address, slot, value)
    }

    fn insert_account_info(&mut self, address: Address, info: AccountInfo) {
        self.insert_account_info(address, info)
    }
}
