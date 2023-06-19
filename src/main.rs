use ::ethers::types::U256;

mod config;
mod engine;
mod utils;

fn main() {
    let a = U256::from(100_u128);
    let b = U256::from(100_u32);
    assert_eq!(a, b);
}
