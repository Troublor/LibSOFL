use criterion::criterion_main;

mod benchmarks;

criterion_main! {
    benchmarks::extract_call::extract_call,
}
