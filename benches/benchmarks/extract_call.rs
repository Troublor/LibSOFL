use std::ops::Range;

use criterion::{criterion_group, Bencher, Criterion};
use libsofl::engine::{
    inspectors::{
        call_extract::CallExtractInspector, no_inspector, MultiTxInspector,
    },
    providers::{BcProvider, BcProviderBuilder},
    state::{env::TransitionSpecBuilder, BcState, BcStateBuilder, ForkedState},
};
use reth_primitives::BlockHashOrNumber;
use revm::Database;

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

criterion_group!(
    extract_call,
    reproduce_blocks_with_inspector,
    reproduce_blocks
);
