use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, consistency);

criterion_main!(benches);

fn consistency(bench: &mut Criterion) {
    let mut group = bench.benchmark_group("consistency");
    let runtime = rt();

    let mut consumer = runtime.block_on(setup());
    runtime.spawn(producer());
    group.bench_function(BenchmarkId::new("test", format!("test")), |b| {
        b.iter(|| {
            runtime.block_on(async { consumer.run().await });
        });
    });
}

struct Consumer {}

impl Consumer {
    async fn run(&mut self) {}
}

async fn setup() -> Consumer {
    Consumer {}
}

async fn producer() {}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
