use ::ethers::types::U256;

#[cfg(test)]
mod poc;

fn main() {
    let a = U256::from(100_u128);
    let b = U256::from(100_u32);
    assert_eq!(a, b);
}
