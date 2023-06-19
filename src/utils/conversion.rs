use std::ops::{Range, RangeBounds, RangeInclusive};

use ethers::types::{
    Address as ethersAddress, Block as ethersBlock, BlockId as ethersBlockId,
    BlockNumber as ethersBlockNumber, Transaction as ethersTransaction,
    TransactionReceipt as ethersReceipt, TxHash as ethersTxHash,
    H256 as ethersH256, U256 as ethersU256, U64 as ethersU64,
};
use reth_primitives::{
    Address, BlockHash, BlockHashOrNumber, Bloom, Header, SealedHeader,
    TransactionMeta, TransactionSigned,
};
use reth_rlp::Decodable;
use revm_primitives::{B256, B256 as H256, U256};

pub trait Convert<F, T> {
    /// Convert from F to To
    fn cvt(v: F) -> T;
}

pub struct FromEthers {}

impl Convert<ethersU256, U256> for FromEthers {
    fn cvt(v: ethersU256) -> U256 {
        let mut b: [u8; 32] = [0; 32];
        v.to_big_endian(&mut b);
        U256::from_be_bytes(b)
    }
}

impl Convert<ethersBlock<ethersTxHash>, Option<SealedHeader>> for FromEthers {
    fn cvt(block: ethersBlock<ethersTxHash>) -> Option<SealedHeader> {
        if block.author.is_none() {
            // return None if the block is still pending
            return None;
        }
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

impl Convert<&ethersTransaction, TransactionSigned> for FromEthers {
    fn cvt(tx: &ethersTransaction) -> TransactionSigned {
        let rlp = tx.rlp();
        let mut rlp = rlp.as_ref();
        TransactionSigned::decode(&mut rlp).unwrap()
    }
}

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
