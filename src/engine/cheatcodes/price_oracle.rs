use reth_primitives::Address;
use revm_primitives::U256;

use crate::{engine::state::BcState, error::SoflError};

use super::CheatCodes;

#[derive(Debug)]
pub struct PriceOracle<S: BcState> {
    tokens: Vec<Address>,
    weth: Address,
    contract_addr: Address,
    pricing: fn(
        &mut CheatCodes<S>,
        &mut S,
        Address,    // token under inspection
        &Address,   // weth
        &[Address], // mainstream tokens
    ) -> Result<U256, SoflError<S::DbErr>>,
}

impl<S: BcState> PriceOracle<S> {
    pub fn get_price_in_ether(
        &mut self,
        cheatcode: &mut CheatCodes<S>,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        (self.pricing)(cheatcode, state, token, &self.weth, &self.tokens)
    }
}

impl<S: BcState> PriceOracle<S> {
    pub fn create_const_oracle() -> Self {
        let const_pricing = |_: &mut CheatCodes<S>,
                             _: &mut S,
                             _: Address,
                             _: &Address,
                             _: &[Address]|
         -> Result<U256, SoflError<S::DbErr>> {
            Ok(U256::from(1_000_000_000_000_000_000u128))
        };

        Self {
            tokens: vec![],
            weth: Address::zero(),
            contract_addr: Address::zero(),
            pricing: const_pricing,
        }
    }
}

pub trait PriceOracleCheat<S: BcState> {
    fn get_price_in_ether(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>>;
}

impl<S: BcState> PriceOracleCheat<S> for CheatCodes<S> {
    fn get_price_in_ether(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        todo!()
    }
}
