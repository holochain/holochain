//! Reads performed by the kitsune2 op store during gossip and fetch.

use super::configure;
use crate::{fixtures, runtime};
use criterion::{BenchmarkId, Criterion, Throughput};

const SINCE_LIMIT: u32 = 500;
const PRESENT_BATCH: usize = 500;
const WIRE_BATCH: usize = 100;

pub fn gossip(c: &mut Criterion) {
    let rt = runtime();
    let mut g = c.benchmark_group("gossip");
    configure(&mut g);
    for fx in fixtures() {
        let db = fx.db.as_ref();
        // A quarter-arc that does not wrap, and a one-hour time slice in the
        // middle of the fixture's time range.
        let arc_start = u32::MAX / 4;
        let arc_end = u32::MAX / 2;
        let mid = (fx.keys.min_timestamp + fx.keys.max_timestamp) / 2;
        let slice = (mid, mid + 3_600_000_000);
        let raw: Vec<Vec<u8>> = fx
            .keys
            .op_hashes
            .iter()
            .map(|h| h.get_raw_36().to_vec())
            .collect();
        let present: Vec<Vec<u8>> = raw.iter().cycle().take(PRESENT_BATCH).cloned().collect();
        let wire: Vec<Vec<u8>> = raw.iter().take(WIRE_BATCH).cloned().collect();

        g.throughput(Throughput::Elements(1));
        g.bench_with_input(
            BenchmarkId::new("op_hashes_in_time_slice", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.op_hashes_in_time_slice(arc_start, arc_end, slice.0, slice.1)
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("op_ids_since_time_batch_500", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.op_ids_since_time_batch(arc_start, arc_end, mid, SINCE_LIMIT)
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("earliest_authored_timestamp_in_arc", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.earliest_authored_timestamp_in_arc(arc_start, arc_end)
                        .await
                        .unwrap()
                })
            },
        );
        g.throughput(Throughput::Elements(PRESENT_BATCH as u64));
        g.bench_with_input(
            BenchmarkId::new("check_op_hashes_present_500", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.check_op_hashes_present(&present).await.unwrap() })
            },
        );
        g.throughput(Throughput::Elements(WIRE_BATCH as u64));
        g.bench_with_input(
            BenchmarkId::new("get_chain_ops_for_wire_100", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_chain_ops_for_wire(&wire).await.unwrap() })
            },
        );
    }
    g.finish();
}
