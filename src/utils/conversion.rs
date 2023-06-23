use std::ops::{RangeBounds, RangeInclusive};

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
use revm_primitives::{hex, B256, B256 as H256, U256};

pub trait Convert<F, T> {
    /// Convert from F to To
    fn cvt(v: F) -> T;
}

/// ############################
/// ToPrimitive
/// Convert to reth_primitives or revm_primitives types
/// ############################

pub struct ToPrimitive {}

impl Convert<&ethersU256, U256> for ToPrimitive {
    fn cvt(v: &ethersU256) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        v.to_big_endian(&mut b);
        U256::from_be_bytes(b)
    }
}

impl Convert<&ethersAddress, Address> for ToPrimitive {
    fn cvt(v: &ethersAddress) -> Address {
        (*v).into()
    }
}

impl Convert<&ethersBytes, Bytes> for ToPrimitive {
    fn cvt(v: &ethersBytes) -> Bytes {
        v.clone().0.into()
    }
}

impl Convert<&ethersH256, H256> for ToPrimitive {
    fn cvt(v: &ethersH256) -> H256 {
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
        hex::decode_to_slice(v, &mut b).unwrap();
        Address::from_slice(&b)
    }
}

impl Convert<&str, H256> for ToPrimitive {
    /// Convert hex string to hash
    fn cvt(v: &str) -> H256 {
        let mut b: [u8; 32] = [0; 32];
        let v = v.trim_start_matches("0x");
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
        };
        let hash = block.hash.unwrap().0.into();
        Some(SealedHeader { header, hash })
    }
}

impl Convert<&ethersLog, Log> for ToPrimitive {
    fn cvt(v: &ethersLog) -> Log {
        Log {
            address: ToPrimitive::cvt(&v.address),
            topics: v.topics.iter().map(ToPrimitive::cvt).collect(),
            data: ToPrimitive::cvt(&v.data),
        }
    }
}

impl Convert<&ethersReceipt, Receipt> for ToPrimitive {
    fn cvt(v: &ethersReceipt) -> Receipt {
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

impl Convert<&ethersTransaction, TransactionSigned> for ToPrimitive {
    fn cvt(tx: &ethersTransaction) -> TransactionSigned {
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

impl Convert<&B256, ethersH256> for ToEthers {
    fn cvt(v: &B256) -> ethersH256 {
        ethersH256::from_slice(&v.0)
    }
}

impl Convert<&u64, ethersU64> for ToEthers {
    fn cvt(v: &u64) -> ethersU64 {
        ethersU64::from(*v)
    }
}

impl Convert<&Address, ethersAddress> for ToEthers {
    fn cvt(v: &Address) -> ethersAddress {
        ethersAddress::from_slice(v.as_slice())
    }
}

impl Convert<&BlockHashOrNumber, ethersBlockId> for ToEthers {
    fn cvt(v: &BlockHashOrNumber) -> ethersBlockId {
        match v {
            BlockHashOrNumber::Hash(h) => ethersBlockId::Hash(ToEthers::cvt(h)),
            BlockHashOrNumber::Number(n) => ethersBlockId::Number(
                ethersBlockNumber::Number(ToEthers::cvt(n)),
            ),
        }
    }
}

impl Convert<&BlockHash, ethersBlockId> for ToEthers {
    fn cvt(v: &BlockHash) -> ethersBlockId {
        ethersBlockId::Hash(ToEthers::cvt(v))
    }
}

impl Convert<&u64, ethersBlockId> for ToEthers {
    fn cvt(v: &u64) -> ethersBlockId {
        ethersBlockId::Number(ethersBlockNumber::Number(ToEthers::cvt(v)))
    }
}

pub struct ToIterator {}

impl<R: RangeBounds<u64>> Convert<R, RangeInclusive<u64>> for ToIterator {
    fn cvt(rb: R) -> RangeInclusive<u64> {
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
