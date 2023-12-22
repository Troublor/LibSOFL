use std::str::FromStr;

use crate::engine::types::{U256, U128, B256};

use super::ConvertTo;

impl ConvertTo<U256> for u128 {
    fn cvt(&self) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        b[16..].copy_from_slice(&self.to_be_bytes());
        U256::from_be_bytes(b)
    }
}

impl ConvertTo<U256> for &str {
    fn cvt(&self) -> U256 {
        U256::from_str(self).expect("failed to convert u256")
    }
}

impl ConvertTo<U256> for B256 {
    fn cvt(&self) -> U256 {
        U256::from_be_slice(self.as_slice())
    }
}

impl ConvertTo<U256> for u32 {
    fn cvt(&self) -> U256 {
        U256::from(*self as u128)
    }
}

impl ConvertTo<U256> for u8 {
    fn cvt(&self) -> U256 {
        U256::from(*self as u128)
    }
}

impl ConvertTo<u64> for U256 {
    fn cvt(&self) -> u64 {
        let be = self.to_be_bytes_trimmed_vec();
        if be.len() > 8 {
            panic!("U256 too large to fit in u64")
        }
        let mut bytes: [u8; 8] = [0; 8];
        bytes[8 - be.len()..].copy_from_slice(be.as_slice());
        u64::from_be_bytes(bytes)
    }
}

impl ConvertTo<U256> for U128 {
    fn cvt(&self) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        b[16..].copy_from_slice(&self.to_be_bytes::<16>());
        U256::from_be_bytes(b)
    }
}

#[cfg(test)]
mod tests {
    use crate::conversion::ConvertFrom;

    use super::*;

    #[test]
    fn test_cvt() {
        let from = 0u128;
        let to = from.cvt();
        assert_eq!(to, U256::ZERO);
        let to = <U256 as ConvertFrom<u128>>::cvt(from);
        assert_eq!(to, U256::ZERO);
    }

    #[test]
    fn test_cvt_str_to_u256() {
        let from = "0xFF";
        let to: U256 = from.cvt();
        assert_eq!(to, U256::from(0xff));
    }
}
