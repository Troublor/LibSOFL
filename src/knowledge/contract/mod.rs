pub mod abi;
pub mod msg_call;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    ops::{Range, RangeBounds},
};

use auto_impl::auto_impl;
use ethers::abi::{Function, Token};
use reth_primitives::{Address, BlockNumber};

use crate::error::SoflError;

use self::msg_call::MsgCall;

#[auto_impl(&, &mut, Arc, Box, Rc)]
pub trait MsgCallProvider {
    fn get_msg_call_for_contract(
        &self,
        contract: Address,
        block_range: impl RangeBounds<BlockNumber>,
    ) -> Result<Vec<MsgCall>, SoflError>;

    fn get_msg_call_for_function<E>(
        &self,
        contract: Address,
        function: &Function,
        block_range: impl RangeBounds<BlockNumber>,
    ) -> Result<Vec<MsgCall>, SoflError>;
}

pub type ContractKnowledge<K> = HashMap<Address, K>;

#[derive(Clone, Debug)]
pub struct FunctionParamKnowledge<P> {
    /// The seed pool: contract address => (function sighash, param index(0-indexed)) => token
    pub seeds: HashMap<Address, BTreeMap<([u8; 4], usize), HashSet<Token>>>,

    provider: P,
    block_range: Range<BlockNumber>,
}

impl<P> FunctionParamKnowledge<P> {
    pub fn new(p: P, block_range: Range<BlockNumber>) -> Self {
        Self {
            seeds: HashMap::new(),
            provider: p,
            block_range,
        }
    }
}

impl<P: MsgCallProvider> FunctionParamKnowledge<P> {
    fn load_contract(&mut self, contract: Address) -> Result<(), SoflError> {
        let calls = self
            .provider
            .get_msg_call_for_contract(contract, self.block_range.clone())?;
        // let mut knowledge = BTreeMap::new();
        for call in calls {}
        todo!()
    }

    pub fn gen_func_arg(
        &mut self,
        contract: Address,
        func: &Function,
        arg_index: usize,
    ) -> Token {
        if !self.seeds.contains_key(&contract) {}
        todo!()
    }
}
