//! If you are trying to run this example, please make sure to run `make bench microvm_provision` from
//! the `monocore` subdirectory

use criterion::{criterion_group, criterion_main, Criterion};
use std::{process::Command, time::Duration};

//--------------------------------------------------------------------------------------------------
// Benchmark
//--------------------------------------------------------------------------------------------------

fn benchmark_microvm_provision(c: &mut Criterion) {
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
                .status()
                .expect("Failed to execute microvm_nop");
            assert!(output.success());
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(20));
    targets = benchmark_microvm_provision
}
criterion_main!(benches);
