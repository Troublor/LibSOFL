use std::str::FromStr;

use crate::{
    blockchain::tx_position::TxPosition,
    engine::types::{
        Address, BlockHashOrNumber, Bytes, Hash, Signed, TxHashOrPosition, Uint,
    },
};

use super::ConvertTo;

//*** Convert To String */
impl ConvertTo<String> for Hash {
    fn cvt(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}
impl ConvertTo<String> for Address {
    fn cvt(&self) -> String {
        self.to_string()
    }
}
impl ConvertTo<String> for Bytes {
    fn cvt(&self) -> String {
        hex::encode(self)
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<String>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> String {
        self.to_string() // decimal string
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<String>
    for Signed<BITS, LIMBS>
{
    fn cvt(&self) -> String {
        self.to_dec_string() // decimal string
    }
}
impl ConvertTo<String> for BlockHashOrNumber {
    fn cvt(&self) -> String {
        match self {
            BlockHashOrNumber::Hash(hash) => hash.cvt(),
            BlockHashOrNumber::Number(number) => number.to_string(),
        }
    }
}
impl ConvertTo<String> for TxHashOrPosition {
    fn cvt(&self) -> String {
        match self {
            TxHashOrPosition::Hash(hash) => hash.cvt(),
            TxHashOrPosition::Position(pos) => pos.cvt(),
        }
    }
}
impl ConvertTo<String> for TxPosition {
    fn cvt(&self) -> String {
        format!("{}:{}", self.block.cvt(), self.index)
    }
}

///*** Convert to Uint */
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for String
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        // treat the string as a hex string if it starts with 0x, otherwise treat it as a decimal string
        Uint::<BITS, LIMBS>::from_str(self.as_str())
            .expect("failed to convert string to Uint")
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for &str
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        // treat the string as a hex string if it starts with 0x, otherwise treat it as a decimal string
        Uint::<BITS, LIMBS>::from_str(self)
            .expect("failed to convert string to Uint")
    }
}
impl<const B1: usize, const L1: usize, const B2: usize, const L2: usize>
    ConvertTo<Uint<B1, L1>> for Uint<B2, L2>
{
    fn cvt(&self) -> Uint<B1, L1> {
        Uint::<B1, L1>::from_be_slice(self.to_be_bytes_trimmed_vec().as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for Hash
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for Address
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for usize
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.to_be_bytes().as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for u8
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.to_be_bytes().as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for u16
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.to_be_bytes().as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for u32
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.to_be_bytes().as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for u64
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.to_be_bytes().as_slice())
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Uint<BITS, LIMBS>>
    for u128
{
    fn cvt(&self) -> Uint<BITS, LIMBS> {
        Uint::<BITS, LIMBS>::from_be_slice(self.to_be_bytes().as_slice())
    }
}

///*** Convert to Bytes */
impl ConvertTo<Bytes> for String {
    fn cvt(&self) -> Bytes {
        let mut bytes = Vec::new();
        hex::decode_to_slice(self.as_str(), &mut bytes)
            .expect("failed to convert string to Bytes");
        bytes.into()
    }
}
impl ConvertTo<Bytes> for &str {
    fn cvt(&self) -> Bytes {
        let mut bytes = Vec::new();
        hex::decode_to_slice(self, &mut bytes)
            .expect("failed to convert string to Bytes");
        bytes.into()
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Bytes>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> Bytes {
        self.to_be_bytes_vec().into()
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<Bytes>
    for Signed<BITS, LIMBS>
{
    fn cvt(&self) -> Bytes {
        self.into_raw().to_be_bytes_vec().into()
    }
}
impl ConvertTo<Bytes> for Address {
    fn cvt(&self) -> Bytes {
        self.as_slice().to_vec().into()
    }
}
impl ConvertTo<Bytes> for Hash {
    fn cvt(&self) -> Bytes {
        self.as_slice().to_vec().into()
    }
}
impl<const LEN: usize> ConvertTo<Bytes> for [u8; LEN] {
    fn cvt(&self) -> Bytes {
        self.as_slice().to_vec().into()
    }
}
impl ConvertTo<Bytes> for &[u8] {
    fn cvt(&self) -> Bytes {
        self.to_vec().into()
    }
}
impl ConvertTo<Bytes> for Vec<u8> {
    fn cvt(&self) -> Bytes {
        self.as_slice().to_vec().into()
    }
}

///*** Convert to usize */
impl<const BITS: usize, const LIMBS: usize> ConvertTo<usize>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> usize {
        let s: String = self.cvt(); // decimal string
        usize::from_str_radix(&s, 10).expect("failed to convert Uint to usize")
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<u8>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> u8 {
        let s: String = self.cvt(); // decimal string
        u8::from_str_radix(&s, 10).expect("failed to convert Uint to u8")
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<u16>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> u16 {
        let s: String = self.cvt(); // decimal string
        u16::from_str_radix(&s, 10).expect("failed to convert Uint to u16")
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<u32>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> u32 {
        let s: String = self.cvt(); // decimal string
        u32::from_str_radix(&s, 10).expect("failed to convert Uint to u32")
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<u64>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> u64 {
        let s: String = self.cvt(); // decimal string
        u64::from_str_radix(&s, 10).expect("failed to convert Uint to u64")
    }
}
impl<const BITS: usize, const LIMBS: usize> ConvertTo<u128>
    for Uint<BITS, LIMBS>
{
    fn cvt(&self) -> u128 {
        let s: String = self.cvt(); // decimal string
        u128::from_str_radix(&s, 10).expect("failed to convert Uint to u128")
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        conversion::{ConvertFrom, ConvertTo},
        engine::types::U256,
    };

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

    #[test]
    fn test_cvt_uint_to_string() {
        let from = U256::from(0xff);
        assert_eq!(ConvertTo::<String>::cvt(&from), "255".to_string());
    }
}
