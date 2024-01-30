use std::{ops::Range, sync::Arc};

use criterion::{criterion_group, criterion_main, Criterion};
use libsofl_core::{
    blockchain::provider::{BcProvider, BcStateProvider},
    engine::{
        inspector::no_inspector, state::BcState,
        transition::TransitionSpecBuilder,
    },
};
use libsofl_reth::{blockchain::provider::RethProvider, config::RethConfig};
use libsofl_utils::config::Config;

fn run_block(provider: Arc<RethProvider>, bn: u64) {
    let mut state = provider.bc_state_at(bn.into()).unwrap();
    let txs = provider.txs_in_block(bn.into()).unwrap();
    let mut spec_builder =
        TransitionSpecBuilder::default().at_block(&provider, bn.into());
    for tx in txs {
        spec_builder = spec_builder.append_tx(tx);
    }
    let spec = spec_builder.build();
    state.transit(spec, no_inspector()).unwrap();
}

fn run_blocks(provider: Arc<RethProvider>, bns: Range<u64>) {
    for bn in bns {
        run_block(provider.clone(), bn);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("block 18000000..18000010", |b| {
        let provider = RethConfig::must_load().bc_provider().unwrap();
        let provider = Arc::new(provider);
        b.iter(|| run_blocks(provider.clone(), 18000000..18000010))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
