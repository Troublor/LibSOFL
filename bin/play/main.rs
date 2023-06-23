use ethers::types::U256;

fn main() {
    let a = U256::from(100_u128);
    let b = U256::from(100_u32);
    println!("{:?} {:?}", a, b);
}
