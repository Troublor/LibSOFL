use std::str::FromStr;

use crate::{
    blockchain::tx_position::TxPosition,
    engine::types::{
        Address, BlockHash, BlockHashOrNumber, BlockNumber, Bytecode, Bytes,
        Hash, TxHash, TxHashOrPosition, U256,
    },
};

use super::ConvertTo;

//*** Convert to Address */
impl ConvertTo<Address> for String {
    fn cvt(&self) -> Address {
        Address::from_str(self).expect("failed to convert address")
    }
}
impl ConvertTo<Address> for &str {
    fn cvt(&self) -> Address {
        Address::from_str(self).expect("failed to convert address")
    }
}
impl ConvertTo<Address> for U256 {
    fn cvt(&self) -> Address {
        let mut b: [u8; 20] = [0; 20];
        b.copy_from_slice(&self.to_be_bytes::<32>()[12..]);
        Address::from(b)
    }
}
impl ConvertTo<Address> for usize {
    fn cvt(&self) -> Address {
        let u256 = U256::from(*self);
        u256.cvt()
    }
}

//*** Convert to Hash */
impl ConvertTo<Hash> for &str {
    fn cvt(&self) -> Hash {
        Hash::from_str(self).expect("failed to convert hash")
    }
}
impl ConvertTo<Hash> for String {
    fn cvt(&self) -> Hash {
        Hash::from_str(self).expect("failed to convert hash")
    }
}
impl ConvertTo<Hash> for U256 {
    fn cvt(&self) -> Hash {
        Hash::from_slice(self.to_be_bytes::<32>().as_slice())
    }
}
impl ConvertTo<Hash> for usize {
    fn cvt(&self) -> Hash {
        let u256 = U256::from(*self);
        u256.cvt()
    }
}

//*** Convert to BlockHashOrNumber */
impl ConvertTo<BlockHashOrNumber> for String {
    fn cvt(&self) -> BlockHashOrNumber {
        BlockHashOrNumber::Hash(self.cvt())
    }
}
impl ConvertTo<BlockHashOrNumber> for &str {
    fn cvt(&self) -> BlockHashOrNumber {
        BlockHashOrNumber::Hash(self.cvt())
    }
}
impl ConvertTo<BlockHashOrNumber> for U256 {
    fn cvt(&self) -> BlockHashOrNumber {
        BlockHashOrNumber::Number(self.cvt())
    }
}
impl ConvertTo<BlockHashOrNumber> for usize {
    fn cvt(&self) -> BlockHashOrNumber {
        BlockHashOrNumber::Number(*self as u64)
    }
}
impl ConvertTo<BlockHashOrNumber> for BlockHash {
    fn cvt(&self) -> BlockHashOrNumber {
        BlockHashOrNumber::Hash(*self)
    }
}
impl ConvertTo<BlockHashOrNumber> for BlockNumber {
    fn cvt(&self) -> BlockHashOrNumber {
        BlockHashOrNumber::Number(*self)
    }
}

/*** Convert to TxHashOrPosition */
impl ConvertTo<TxHashOrPosition> for String {
    fn cvt(&self) -> TxHashOrPosition {
        TxHashOrPosition::Hash(self.cvt())
    }
}
impl ConvertTo<TxHashOrPosition> for &str {
    fn cvt(&self) -> TxHashOrPosition {
        TxHashOrPosition::Hash(self.cvt())
    }
}
impl ConvertTo<TxHashOrPosition> for U256 {
    fn cvt(&self) -> TxHashOrPosition {
        TxHashOrPosition::Position(TxPosition {
            block: self.cvt(),
            index: 0,
        })
    }
}
impl ConvertTo<TxHashOrPosition> for usize {
    fn cvt(&self) -> TxHashOrPosition {
        TxHashOrPosition::Position(TxPosition {
            block: self.cvt(),
            index: 0,
        })
    }
}
impl ConvertTo<TxHashOrPosition> for TxPosition {
    fn cvt(&self) -> TxHashOrPosition {
        TxHashOrPosition::Position(self.clone())
    }
}
impl ConvertTo<TxHashOrPosition> for TxHash {
    fn cvt(&self) -> TxHashOrPosition {
        TxHashOrPosition::Hash(*self)
    }
}

///*** Convert to Bytecode */
impl<T: ConvertTo<Bytes>> ConvertTo<Bytecode> for T {
    fn cvt(&self) -> Bytecode {
        let bytes: Bytes = self.cvt();
        Bytecode::new_raw(bytes)
    }
}
impl ConvertTo<Bytecode> for Bytes {
    fn cvt(&self) -> Bytecode {
        Bytecode::new_raw(self.clone())
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
        assert_eq!(addr_s, "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");
    }

    #[test]
    fn test_convert_address_to_u256() {
        let addr: Address = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789".cvt();
        let uint: U256 = addr.cvt();
        assert_eq!(uint, "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789".cvt());
    }
}
