use std::{ops::Range, str::FromStr};

use criterion::{criterion_group, Bencher, Criterion};
use libsofl::engine::{
    inspectors::{
        call_extract::CallExtractInspector, no_inspector, MultiTxInspector,
    },
    providers::{BcProvider, BcProviderBuilder},
    state::{env::TransitionSpecBuilder, BcState, BcStateBuilder, ForkedState},
};
use reth_primitives::{BlockHashOrNumber, TxHash};

fn reproduce_block<
    'a,
    P: BcProvider,
    B: Into<BlockHashOrNumber>,
    I: MultiTxInspector<ForkedState<'a>>,
>(
    p: &'a P,
    block: B,
    insp: &mut I,
) {
    let block = block.into();
    let state = BcStateBuilder::fork_at(p, block).unwrap();
    let txs = p.transactions_by_block(block).unwrap();
    let txs = match txs {
        Some(txs) => txs,
        None => return,
    };
    let spec = TransitionSpecBuilder::new()
        .at_block(p, block)
        .append_signed_txs(txs)
        .build();
    let _ = BcState::transit(state, spec, insp).unwrap();
}

pub fn reproduce_blocks_with_inspector(c: &mut Criterion) {
    let mut group = c.benchmark_group("reproduce_blocks_with_inspector");
    let provider = BcProviderBuilder::default_db().unwrap();
    // 100 blocks
    let runner = |b: &mut Bencher, r: &Range<u64>| {
        b.iter(|| {
            let mut insp = CallExtractInspector::default();
            for i in r.clone() {
                reproduce_block(&provider, i, &mut insp);
            }
            assert!(!insp.calls.is_empty());
        })
    };
    group.bench_with_input(
        "block 15000000-15000010",
        &(15000000..15000010),
        runner,
    );
    group.bench_with_input(
        "block 16000000-16000010",
        &(16000000..16000010),
        runner,
    );
    group.bench_with_input(
        "block 17000000-17000010",
        &(17000000..17000010),
        runner,
    );
}

pub fn reproduce_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("reproduce_blocks");
    let provider = BcProviderBuilder::default_db().unwrap();
    // 100 blocks
    let runner = |b: &mut Bencher, r: &Range<u64>| {
        b.iter(|| {
            let mut insp = no_inspector();
            for i in r.clone() {
                reproduce_block(&provider, i, &mut insp);
            }
        })
    };
    group.bench_with_input(
        "block 15000000-15000010",
        &(15000000..15000010),
        runner,
    );
    group.bench_with_input(
        "block 16000000-16000010",
        &(16000000..16000010),
        runner,
    );
    group.bench_with_input(
        "block 17000000-17000010",
        &(17000000..17000010),
        runner,
    );
}

pub fn reproduce_tx<
    'a,
    P: BcProvider,
    T: Into<TxHash>,
    I: MultiTxInspector<ForkedState<'a>>,
>(
    p: &'a P,
    tx: T,
    insp: &mut I,
) {
    let (tx, tx_meta) = p
        .transaction_by_hash_with_meta(tx.into())
        .unwrap()
        .expect("tx not found");
    let spec = TransitionSpecBuilder::default()
        .at_block(p, tx_meta.block_number)
        .append_signed_tx(tx)
        .build();
    let state =
        BcStateBuilder::fork_at(p, (tx_meta.block_number, tx_meta.index))
            .unwrap();
    let _ = BcState::transit(state, spec, insp).unwrap();
}

pub fn reproduce_very_large_tx(c: &mut Criterion) {
    let mut group = c.benchmark_group("reproduce_very_large_tx");
    let provider = BcProviderBuilder::default_db().unwrap();
    let runner = |b: &mut Bencher, tx: &str| {
        b.iter(|| {
            let mut insp = no_inspector();
            reproduce_tx(&provider, TxHash::from_str(tx).unwrap(), &mut insp);
        })
    };
    group.bench_with_input(
        "tx 0x0fe2542079644e107cbf13690eb9c2c65963ccb79089ff96bfaf8dced2331c92",
        "0x0fe2542079644e107cbf13690eb9c2c65963ccb79089ff96bfaf8dced2331c92",
        runner,
    );
}

pub fn reproduce_very_large_tx_with_inspector(c: &mut Criterion) {
    let mut group = c.benchmark_group("reproduce_very_large_tx_with_inspector");
    let provider = BcProviderBuilder::default_db().unwrap();
    let runner = |b: &mut Bencher, tx: &str| {
        b.iter(|| {
            let mut insp = CallExtractInspector::default();
            reproduce_tx(&provider, TxHash::from_str(tx).unwrap(), &mut insp);
            assert!(!insp.calls.is_empty());
        })
    };
    group.bench_with_input(
        "tx 0x0fe2542079644e107cbf13690eb9c2c65963ccb79089ff96bfaf8dced2331c92",
        "0x0fe2542079644e107cbf13690eb9c2c65963ccb79089ff96bfaf8dced2331c92",
        runner,
    );
}

criterion_group!(
    extract_call,
    reproduce_blocks_with_inspector,
    reproduce_blocks,
    reproduce_very_large_tx,
    reproduce_very_large_tx_with_inspector
);
