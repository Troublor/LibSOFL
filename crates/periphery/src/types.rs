pub type Chain = ethers::types::Chain;

// Alloy's Solidity types
pub type SolUint<const BITS: usize> = alloy_sol_types::sol_data::Uint<BITS>;
pub type SolUint256 = SolUint<256>;
pub type SolAddress = alloy_sol_types::sol_data::Address;
