use std::ops::{RangeBounds, RangeInclusive};

use ethers::abi::{RawLog as ethersRawLog, Token};
use ethers::types::{
    Address as ethersAddress, Block as ethersBlock, BlockId as ethersBlockId,
    BlockNumber as ethersBlockNumber, Bytes as ethersBytes, Log as ethersLog,
    Transaction as ethersTransaction, TransactionReceipt as ethersReceipt,
    TxHash as ethersTxHash, H256 as ethersH256, U256 as ethersU256,
    U64 as ethersU64,
};

use reth_primitives::{
    Address, BlockHash, BlockHashOrNumber, Bloom, Bytecode, Bytes, Header, Log,
    Receipt, SealedHeader, TransactionSigned, TxType,
};

use reth_rlp::Decodable;
use revm_primitives::{hex, Bytes as revmBytes, B256, B256 as H256, U256};

pub trait Convert<F, T> {
    /// Convert from F to To
    fn cvt(v: F) -> T;
}

impl<F: Clone, T, C: Convert<F, T>> Convert<&F, T> for C {
    /// Convert from F to To
    fn cvt(v: &F) -> T {
        C::cvt(v.clone())
    }
}

pub struct ToElementary {}

impl Convert<U256, u64> for ToElementary {
    /// convert U256 to u64.
    /// Panics if the value is too large to fit in a u64
    fn cvt(v: U256) -> u64 {
        let be = v.to_be_bytes_trimmed_vec();
        if be.len() > 8 {
            panic!("U256 too large to fit in u64")
        }
        let mut bytes: [u8; 8] = [0; 8];
        bytes[8 - be.len()..].copy_from_slice(be.as_slice());
        u64::from_be_bytes(bytes)
    }
}

impl Convert<U256, u128> for ToElementary {
    /// convert U256 to u128.
    /// Panics if the value is too large to fit in a u128
    fn cvt(v: U256) -> u128 {
        let be = v.to_be_bytes_trimmed_vec();
        if be.len() > 16 {
            panic!("U256 too large to fit in u128")
        }
        let mut bytes: [u8; 16] = [0; 16];
        bytes[16 - be.len()..].copy_from_slice(be.as_slice());
        u128::from_be_bytes(bytes)
    }
}

/// ############################
/// ToPrimitive
/// Convert to reth_primitives or revm_primitives types
/// ############################

pub struct ToPrimitive {}

impl Convert<ethersU256, U256> for ToPrimitive {
    fn cvt(v: ethersU256) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        v.to_big_endian(&mut b);
        U256::from_be_bytes(b)
    }
}

impl Convert<ethersU256, B256> for ToPrimitive {
    fn cvt(v: ethersU256) -> B256 {
        let mut b: [u8; 32] = [0; 32];
        v.to_big_endian(&mut b);
        B256::from_slice(&b)
    }
}

impl Convert<ethersAddress, Address> for ToPrimitive {
    fn cvt(v: ethersAddress) -> Address {
        v.into()
    }
}

impl Convert<ethersBytes, Bytes> for ToPrimitive {
    fn cvt(v: ethersBytes) -> Bytes {
        v.0.into()
    }
}

impl Convert<revmBytes, Bytes> for ToPrimitive {
    fn cvt(v: revmBytes) -> Bytes {
        v.as_ref().into()
    }
}

impl Convert<ethersH256, H256> for ToPrimitive {
    fn cvt(v: ethersH256) -> H256 {
        v.0.into()
    }
}

impl Convert<&[u8], Bytecode> for ToPrimitive {
    fn cvt(v: &[u8]) -> Bytecode {
        let bytes = Bytes::from(v);
        Bytecode::new_raw(bytes.0)
    }
}

impl Convert<&str, U256> for ToPrimitive {
    fn cvt(v: &str) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        let v = v.trim_start_matches("0x");
        hex::decode_to_slice(v, &mut b).unwrap();
        U256::from_be_bytes(b)
    }
}

impl Convert<&str, Address> for ToPrimitive {
    /// Convert hex string to address
    fn cvt(v: &str) -> Address {
        let mut b: [u8; 20] = [0; 20];
        let v = v.trim_start_matches("0x");
        let v = format!("{:0>40}", v);
        hex::decode_to_slice(v, &mut b).unwrap();
        Address::from_slice(&b)
    }
}

impl Convert<&str, H256> for ToPrimitive {
    /// Convert hex string to hash
    fn cvt(v: &str) -> H256 {
        let mut b: [u8; 32] = [0; 32];
        let v = v.trim_start_matches("0x");
        let v = format!("{:0>64}", v);
        hex::decode_to_slice(v, &mut b).unwrap();
        H256::from_slice(&b)
    }
}

impl Convert<&str, Bytes> for ToPrimitive {
    fn cvt(v: &str) -> Bytes {
        let v = v.trim_start_matches("0x");
        let b = hex::decode(v).unwrap();
        b.into()
    }
}

impl Convert<u64, Address> for ToPrimitive {
    fn cvt(v: u64) -> Address {
        let mut b: [u8; 20] = [0; 20];
        b[12..].copy_from_slice(&v.to_be_bytes());
        Address::from_slice(&b)
    }
}

impl Convert<u128, U256> for ToPrimitive {
    fn cvt(v: u128) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        b[16..].copy_from_slice(&v.to_be_bytes());
        U256::from_be_bytes(b)
    }
}

impl Convert<ethersBlock<ethersTxHash>, Option<SealedHeader>> for ToPrimitive {
    fn cvt(block: ethersBlock<ethersTxHash>) -> Option<SealedHeader> {
        block.author?;
        let header = Header {
            parent_hash: H256::from_slice(block.parent_hash.as_bytes()),
            ommers_hash: H256::from_slice(block.uncles_hash.as_bytes()),
            beneficiary: Address::from_slice(block.author.unwrap().as_bytes()),
            state_root: H256::from_slice(block.state_root.as_bytes()),
            transactions_root: H256::from_slice(
                block.transactions_root.as_bytes(),
            ),
            receipts_root: H256::from_slice(block.receipts_root.as_bytes()),
            withdrawals_root: block
                .withdrawals_root
                .map(|r| H256::from_slice(r.as_bytes())),
            logs_bloom: Bloom::from_slice(block.logs_bloom.unwrap().as_bytes()),
            difficulty: U256::from_be_bytes(block.difficulty.into()),
            number: block.number.unwrap().as_u64(),
            gas_limit: block.gas_limit.as_u64(),
            gas_used: block.gas_used.as_u64(),
            timestamp: block.timestamp.as_u64(),
            mix_hash: H256::from_slice(block.mix_hash.unwrap().as_bytes()),
            nonce: block.nonce.unwrap().to_low_u64_be(), // TODO: check whether big-endian is
            // correct
            base_fee_per_gas: block.base_fee_per_gas.map(|f| f.as_u64()),
            extra_data: block.extra_data.0.into(),
            blob_gas_used: None, // TODO: check if this is correct
            excess_blob_gas: None, // TODO: check if this is correct
            parent_beacon_block_root: None, // TODO: check if this is correct
        };
        let hash = block.hash.unwrap().0.into();
        Some(SealedHeader { header, hash })
    }
}

impl Convert<ethersLog, Log> for ToPrimitive {
    fn cvt(v: ethersLog) -> Log {
        Log {
            address: ToPrimitive::cvt(v.address),
            topics: v.topics.iter().map(ToPrimitive::cvt).collect(),
            data: ToPrimitive::cvt(v.data),
        }
    }
}

impl Convert<ethersReceipt, Receipt> for ToPrimitive {
    fn cvt(v: ethersReceipt) -> Receipt {
        let tx_type = match v.transaction_type.map(|t| t.as_u64()) {
            None => TxType::Legacy,
            Some(1) => TxType::EIP2930,
            Some(2) => TxType::EIP1559,
            Some(n) => panic!("Unsupported transaction type: {}", n),
        };
        let success = match v.status.map(|s| s.as_u64()) {
            None => true,
            Some(0) => false,
            Some(1) => true,
            Some(n) => panic!("Invalid status: {}", n),
        };
        Receipt {
            tx_type,
            success,
            cumulative_gas_used: v.cumulative_gas_used.as_u64(),
            logs: v.logs.iter().map(ToPrimitive::cvt).collect(),
        }
    }
}

impl Convert<ethersTransaction, TransactionSigned> for ToPrimitive {
    fn cvt(tx: ethersTransaction) -> TransactionSigned {
        let rlp = tx.rlp();
        let mut rlp = rlp.as_ref();
        TransactionSigned::decode(&mut rlp).unwrap()
    }
}

/// ############################
/// ToEthers
/// Convert to ethers-rsdata types
/// ############################

pub struct ToEthers {}

impl Convert<Bytes, ethersBytes> for ToEthers {
    fn cvt(v: Bytes) -> ethersBytes {
        ethersBytes::from(v.0)
    }
}

impl Convert<U256, ethersU256> for ToEthers {
    fn cvt(v: U256) -> ethersU256 {
        ethersU256::from(v)
    }
}

impl Convert<B256, ethersH256> for ToEthers {
    fn cvt(v: B256) -> ethersH256 {
        ethersH256::from_slice(&v.0)
    }
}

impl Convert<u64, ethersU64> for ToEthers {
    fn cvt(v: u64) -> ethersU64 {
        ethersU64::from(v)
    }
}

impl Convert<u128, ethersU256> for ToEthers {
    fn cvt(v: u128) -> ethersU256 {
        ethersU256::from(v)
    }
}

impl Convert<Address, ethersAddress> for ToEthers {
    fn cvt(v: Address) -> ethersAddress {
        ethersAddress::from_slice(v.as_slice())
    }
}

impl Convert<BlockHashOrNumber, ethersBlockId> for ToEthers {
    fn cvt(v: BlockHashOrNumber) -> ethersBlockId {
        match v {
            BlockHashOrNumber::Hash(h) => ethersBlockId::Hash(ToEthers::cvt(h)),
            BlockHashOrNumber::Number(n) => ethersBlockId::Number(
                ethersBlockNumber::Number(ToEthers::cvt(n)),
            ),
        }
    }
}

impl Convert<BlockHash, ethersBlockId> for ToEthers {
    fn cvt(v: BlockHash) -> ethersBlockId {
        ethersBlockId::Hash(ToEthers::cvt(v))
    }
}

impl Convert<u64, ethersBlockId> for ToEthers {
    fn cvt(v: u64) -> ethersBlockId {
        ethersBlockId::Number(ethersBlockNumber::Number(ToEthers::cvt(v)))
    }
}

impl Convert<Log, ethersRawLog> for ToEthers {
    fn cvt(v: Log) -> ethersRawLog {
        ethersRawLog {
            topics: v.topics.iter().map(ToEthers::cvt).collect(),
            data: v.data.to_vec(),
        }
    }
}

impl Convert<Address, Token> for ToEthers {
    fn cvt(v: Address) -> Token {
        Token::Address(ToEthers::cvt(v))
    }
}

impl Convert<U256, Token> for ToEthers {
    fn cvt(v: U256) -> Token {
        Token::Uint(ToEthers::cvt(v))
    }
}

impl Convert<u128, Token> for ToEthers {
    fn cvt(v: u128) -> Token {
        Token::Uint(ToEthers::cvt(v))
    }
}

impl<T> Convert<Vec<T>, Token> for ToEthers
where
    ToEthers: Convert<T, Token>,
{
    fn cvt(v: Vec<T>) -> Token {
        Token::FixedArray(v.into_iter().map(|t| ToEthers::cvt(t)).collect())
    }
}

pub struct ToIterator {}

impl ToIterator {
    pub fn from_range_bounds<R: RangeBounds<u64>>(
        rb: R,
    ) -> RangeInclusive<u64> {
        let start: u64 = match rb.start_bound() {
            std::ops::Bound::Included(v) => *v,
            std::ops::Bound::Excluded(v) => v + 1,
            std::ops::Bound::Unbounded => u64::MIN,
        };
        let end: u64 = match rb.end_bound() {
            std::ops::Bound::Included(v) => *v,
            std::ops::Bound::Excluded(v) => v - 1,
            std::ops::Bound::Unbounded => u64::MAX,
        };
        start..=end
    }
}
