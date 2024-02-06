use libafl_bolts::rands::Rand;
use libsofl_core::{
    conversion::ConvertTo,
    engine::types::{Address, U256},
};

use super::pentry::RandomlyGeneratable;

impl<R: Rand> RandomlyGeneratable<R> for Address {
    fn generate(rand: &mut R) -> Self {
        let v1: U256 = rand.next().cvt();
        let v2: U256 = rand.next().cvt();
        let v3: U256 = rand.next().cvt();
        let d = v1 * v2 * v3;
        d.cvt()
    }
}

impl<R: Rand> RandomlyGeneratable<R> for U256 {
    fn generate(rand: &mut R) -> Self {
        let v1: U256 = rand.next().cvt();
        let v2: U256 = rand.next().cvt();
        let v3: U256 = rand.next().cvt();
        let v4: U256 = rand.next().cvt();
        let d = v1 * v2 * v3 * v4;
        d
    }
}
