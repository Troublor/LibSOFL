use std::str::FromStr;

use alloy_primitives::U64;

use crate::engine::types::{Address, Bytes, Hash, U256};

use super::ConvertTo;

impl ConvertTo<Address> for &str {
    fn cvt(&self) -> Address {
        Address::from_str(self).expect("failed to convert address")
    }
}

impl ConvertTo<String> for Address {
    fn cvt(&self) -> String {
        format!("0x{}", hex::encode(self.0.0))
    }
}

impl ConvertTo<Hash> for &str {
    fn cvt(&self) -> Hash {
        Hash::from_str(self).expect("failed to convert hash")
    }
}

impl ConvertTo<String> for Hash {
    fn cvt(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}

impl ConvertTo<Bytes> for &[u8] {
    fn cvt(&self) -> Bytes {
        self.to_vec().into()
    }
}

impl ConvertTo<Bytes> for Vec<u8> {
    fn cvt(&self) -> Bytes {
        self.clone().into()
    }
}

impl ConvertTo<u64> for U64 {
    fn cvt(&self) -> u64 {
        u64::from_be_bytes(self.to_be_bytes())
    }
}

impl ConvertTo<Address> for U256 {
    fn cvt(&self) -> Address {
        let mut b: [u8; 20] = [0; 20];
        b.copy_from_slice(&self.to_be_bytes::<32>()[12..]);
        Address::from(b)
    }
}

impl ConvertTo<U256> for Address {
    fn cvt(&self) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        b[12..].copy_from_slice(self.0.0.as_slice());
        U256::from_be_bytes(b)
    }
}

impl ConvertTo<Bytes> for U256 {
    fn cvt(&self) -> Bytes {
        self.to_be_bytes::<32>().to_vec().into()
    }
}

impl ConvertTo<U256> for usize {
    fn cvt(&self) -> U256 {
        U256::from(*self)
    }
}

impl ConvertTo<Address> for usize {
    fn cvt(&self) -> Address {
        let u256 = U256::from(*self);
        u256.cvt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_convert_u256_to_address() {
        let uint: U256 = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789".cvt();
        let addr: Address = uint.cvt();
        let addr_s: String = addr.cvt();
        assert_eq!(addr_s, "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789".to_lowercase().to_string());
    }

    #[test]
    fn test_convert_address_to_u256() {
        let addr: Address = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789".cvt();
        let uint: U256 = addr.cvt();
        assert_eq!(uint, "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789".cvt());
    }
}
