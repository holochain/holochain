//! Reads performed on every root zome call.

use super::{configure, Rotate};
use crate::{fixtures, runtime};
use criterion::{BenchmarkId, Criterion, Throughput};

pub fn zome_call(c: &mut Criterion) {
    let rt = runtime();
    let mut g = c.benchmark_group("zome_call");
    configure(&mut g);
    for fx in fixtures() {
        let db = fx.db.as_ref();
        let authors = Rotate::new(&fx.keys.authors);
        let actions = Rotate::new(&fx.keys.action_hashes);
        let local = &fx.keys.local_author;
        g.throughput(Throughput::Elements(1));

        g.bench_with_input(
            BenchmarkId::new("chain_head_for_author", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.chain_head_for_author(authors.next()).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_actions_by_author", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_actions_by_author(authors.next().clone())
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_cap_grants_by_access", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_cap_grants_by_access(local.clone(), 0).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_action_pk_control", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_action(actions.next().clone()).await.unwrap() })
            },
        );
    }
    g.finish();
}
