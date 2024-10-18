use criterion::{criterion_group, criterion_main, Criterion};
use std::{process::Command, time::Duration};

//--------------------------------------------------------------------------------------------------
// Benchmark
//--------------------------------------------------------------------------------------------------

fn benchmark_microvm_nop(c: &mut Criterion) {
    // First, execute the make command
    let make_status = Command::new("make")
        .args(&["example", "microvm_nop"])
        .status()
        .expect("Failed to execute make command");

    if !make_status.success() {
        panic!("make example microvm_nop failed");
    }

    // Now benchmark the microvm_nop example
    c.bench_function("microvm_nop", |b| {
        b.iter(|| {
            let output = Command::new("../target/release/examples/microvm_nop")
                .output()
                .expect("Failed to execute microvm_nop");
            assert!(output.status.success());
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(10));
    targets = benchmark_microvm_nop
}
criterion_main!(benches);
