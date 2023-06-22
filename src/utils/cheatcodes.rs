// A set of cheatcodes that can directly modify the environments

use crate::engine::executor::{Executor, ExecutorError};
use revm::{db::DatabaseRef, Database, DatabaseCommit};
use revm_primitives::{Address, Bytecode, U256};

#[derive(Debug)]
pub enum CheatCodeError<DBERR> {
    Executor(ExecutorError<DBERR>),
    Database(DBERR),
    AccountNotFound(Address),
}

impl<DBERR> From<ExecutorError<DBERR>> for CheatCodeError<DBERR> {
    fn from(e: ExecutorError<DBERR>) -> Self {
        CheatCodeError::Executor(e)
    }
}

impl<DBERR> From<DBERR> for CheatCodeError<DBERR> {
    fn from(e: DBERR) -> Self {
        CheatCodeError::Database(e)
    }
}

pub fn get_balance<S: DatabaseRef>(
    executor: &Executor<S>,
    address: Address,
) -> Result<U256, CheatCodeError<S::Error>> {
    executor
        .get_state()
        .basic(address)?
        .map(|info| info.balance)
        .ok_or(CheatCodeError::AccountNotFound(address))
}

pub fn get_code<S: DatabaseRef>(
    executor: &Executor<S>,
    address: Address,
) -> Result<Option<Bytecode>, CheatCodeError<S::Error>> {
    executor
        .get_state()
        .basic(address)?
        .map(|info| info.code)
        .ok_or(CheatCodeError::AccountNotFound(address))
}

pub fn get_token_balance<S: DatabaseRef>(
    executor: &Executor<S>,
    token: Address,
    address: Address,
) -> Result<U256, CheatCodeError<S::Error>> {
    executor
        .get_state()
        .erc20(token)?
        .map(|info| info.balance(address))
        .ok_or(CheatCodeError::AccountNotFound(address))
}
