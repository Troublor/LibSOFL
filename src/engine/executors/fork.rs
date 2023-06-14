//! This module defines ForkExecutor which execute transactions on a forked blockchain state.

use std::{path::Path, sync::Arc};

use reth_db::database::Database;
use reth_interfaces::blockchain_tree::BlockchainTreeViewer;
use reth_interfaces::Error as rethError;
use reth_primitives::{BlockHashOrNumber, BlockId, Receipt, TransactionSigned};
use reth_provider::{
    providers::BlockchainProvider, BlockchainTreePendingStateProvider, EvmEnvProvider,
    StateProvider, StateProviderFactory, TransactionsProvider,
};
use reth_revm::{
    database::{State, SubState},
    env::fill_tx_env,
};
use reth_rpc::eth::error::EthApiError;
use revm::{db::CacheDB, EVM};
use revm_primitives::{BlockEnv, CfgEnv, Env, ExecutionResult, TxEnv, B256};

use crate::engine::{
    builders::{BcDB, BcTree, BlockchainProviderBuilder},
    executor::{Executor, ExecutorError},
};
pub struct TxPosition {
    block: BlockHashOrNumber,
    index: u64,
}

impl TxPosition {
    pub fn new(block: u64, index: u64) -> Self {
        Self {
            block: BlockHashOrNumber::from(block),
            index,
        }
    }

    pub fn from(block: B256, index: u64) -> Self {
        Self {
            block: BlockHashOrNumber::from(block),
            index,
        }
    }
}

// type FEvm = EVM<mut CacheDB<State<Box<dyn StateProvider>>>>;

pub struct ForkExecutor<'a, Z: 'a> {
    phantom: std::marker::PhantomData<&'a Z>,
    /// provider is the blockchain provider of the forked blockchain.
    // pub provider: &'a BlockchainProvider<D, T>,

    /// pos is the position of the transaction whose post-state is forked.
    pub pos: TxPosition,

    /// env is the evm environment of next transaction.
    /// This is updated continuously as transactions are executed.
    block_env: BlockEnv,
    cfg: CfgEnv,

    // state: CacheDB<State<Box<dyn StateProvider>>>,
    sp: Box<dyn StateProvider>,
}

impl<'a, T: 'a> ForkExecutor<'a>
// where
//     D: Database,
//     T: BlockchainTreeViewer + BlockchainTreePendingStateProvider,
{
    pub fn new<D: Database, T: BlockchainTreeViewer + BlockchainTreePendingStateProvider>(
        provider: &'a BlockchainProvider<D, T>,
        pos: TxPosition,
    ) -> Option<Self> {
        // verify that position is valid, i.e., in history
        let txs = provider.transactions_by_block(pos.block).unwrap()?;
        if txs.len() <= pos.index as usize {
            return None;
        }
        let mut cfg = CfgEnv::default();
        let mut block_env = BlockEnv::default();
        provider.fill_env_at(&mut cfg, &mut block_env, pos.block);
        let state_provider = provider
            .state_by_block_id(BlockId::from(pos.block))
            .unwrap();
        // let st = State::new(state_provider);
        // let state = SubState::new(st);
        Some(Self {
            // provider,
            phantom: std::marker::PhantomData,
            pos,
            block_env,
            cfg,
            sp: state_provider,
        })
    }
    // pub fn new(
    //     provider: &'a BlockchainProvider<D, T>,
    //     pos: TxPosition,
    // ) -> State<Box<dyn StateProvider>> {
    //     // verify that position is valid, i.e., in history
    //     let txs = provider.transactions_by_block(pos.block).unwrap().unwrap();
    //     let mut cfg = CfgEnv::default();
    //     let mut block_env = BlockEnv::default();
    //     provider.fill_env_at(&mut cfg, &mut block_env, pos.block);
    //     let state_provider = provider
    //         .state_by_block_id(BlockId::from(pos.block))
    //         .unwrap();
    //     let st = State::new(state_provider);
    //     // let state = SubState::new(st);
    //     return st;
    // }
}

// impl ForkExecutor<'_, BcDB, BcTree> {
//     pub fn from_mainnet(datadir: &Path, pos: TxPosition) -> Result<Option<Self>, rethError> {
//         let builder = BlockchainProviderBuilder::mainnet();
//         let provider = builder.with_existing_db(datadir)?;
//         Ok(Self::new(provider, pos))
//     }
// }

// impl<D, T> ForkExecutor<'_, D, T>
// where
//     D: Database,
//     T: BlockchainTreeViewer + BlockchainTreePendingStateProvider,
// {
//     fn run(&mut self, tx: TransactionSigned) -> Result<(ExecutionResult, Receipt), ExecutorError> {
//         let mut tx_env = TxEnv::default();
//         let signer = tx
//             .recover_signer()
//             .ok_or_else(|| ExecutorError::InvalidTransactionError)?;
//         fill_tx_env(&mut tx_env, tx, signer);
//         let env = Env {
//             cfg: self.cfg.clone(),
//             block: self.block_env.clone(),
//             tx: tx_env,
//         };
//         let mut evm = revm::EVM::with_env(env);
//         evm.database(&mut self.state);
//         todo!()
//     }
// }

// impl<D, T> ForkExecutor<'_, D, T>
// where
//     D: Database,
//     T: BlockchainTreeViewer + BlockchainTreePendingStateProvider,
// {
//     fn simulate(&self, tx: TransactionSigned) -> Result<(ExecutionResult, Receipt), ExecutorError> {
//         todo!()
//     }

//     fn execute(&self, tx: TransactionSigned) -> Result<(ExecutionResult, Receipt), ExecutorError> {
//         todo!()
//     }

//     fn env(&self) -> Env {
//         todo!()
//     }

//     fn commit_block(&self) {
//         todo!()
//     }
// }
