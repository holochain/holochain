//! Local cascade reads behind `get`, `get_details` and `get_links`.

use super::{configure, Rotate};
use crate::{fixtures, runtime};
use criterion::{BenchmarkId, Criterion, Throughput};

pub fn cascade_local(c: &mut Criterion) {
    let rt = runtime();
    let mut g = c.benchmark_group("cascade_local");
    configure(&mut g);
    for fx in fixtures() {
        let db = fx.db.as_ref();
        let entries = Rotate::new(&fx.keys.entry_hashes);
        let updated = Rotate::new(&fx.keys.updated_action_hashes);
        let deleted = Rotate::new(&fx.keys.deleted_action_hashes);
        let bases = Rotate::new(&fx.keys.link_bases);
        let links = Rotate::new(&fx.keys.create_link_hashes);
        let actions = Rotate::new(&fx.keys.action_hashes);
        g.throughput(Throughput::Elements(1));

        g.bench_with_input(
            BenchmarkId::new("get_live_entry_creates", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_live_entry_creates(entries.next(), None).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_update_actions_for_record", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_update_actions_for_record(updated.next()).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_delete_actions_for_record", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_delete_actions_for_record(deleted.next()).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_live_link_actions", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_live_link_actions(bases.next()).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_delete_link_actions", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_delete_link_actions(links.next()).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_chain_ops_for_action", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_chain_ops_for_action(actions.next().clone()).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_deleted_records", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_deleted_records(deleted.next().clone()).await.unwrap() })
            },
        );
    }
    g.finish();
}
