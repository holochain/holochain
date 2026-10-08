//! Reads performed when serving other peers' get requests.

use super::{configure, Rotate};
use crate::{fixtures, runtime};
use criterion::{BenchmarkId, Criterion, Throughput};

pub fn authority(c: &mut Criterion) {
    let rt = runtime();
    let mut g = c.benchmark_group("authority");
    configure(&mut g);
    for fx in fixtures() {
        let db = fx.db.as_ref();
        let actions = Rotate::new(&fx.keys.action_hashes);
        let entries = Rotate::new(&fx.keys.entry_hashes);
        let bases = Rotate::new(&fx.keys.link_bases);
        let authors = Rotate::new(&fx.keys.authors);
        let warrantees = Rotate::new(&fx.keys.warrantees);
        g.throughput(Throughput::Elements(1));

        g.bench_with_input(
            BenchmarkId::new("get_authority_store_record", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_authority_store_record(actions.next()).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_authority_entry_creates", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_authority_entry_creates(entries.next()).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_authority_link_creates", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_authority_link_creates(bases.next()).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_authority_delete_links", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_authority_delete_links(bases.next()).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_agent_activity", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_agent_activity(authors.next().clone(), false).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_filtered_agent_activity", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_filtered_agent_activity(authors.next().clone(), u32::MAX, None)
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_warrants_by_warrantee", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.get_warrants_by_warrantee(warrantees.next().clone())
                        .await
                        .unwrap()
                })
            },
        );
    }
    g.finish();
}
