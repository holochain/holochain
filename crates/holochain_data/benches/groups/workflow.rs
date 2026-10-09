//! Reads performed by the validation, integration, publish and receipt
//! workflows.

use super::{configure, Rotate};
use crate::{fixtures, runtime};
use criterion::{BenchmarkId, Criterion, Throughput};

const WORKFLOW_LIMIT: u32 = 10_000;
const PRESENT_BATCH: usize = 100;

pub fn workflow(c: &mut Criterion) {
    let rt = runtime();
    let mut g = c.benchmark_group("workflow");
    configure(&mut g);
    for fx in fixtures() {
        let db = fx.db.as_ref();
        let actions = Rotate::new(&fx.keys.action_hashes);
        let outcome_integrated = Rotate::new(&fx.keys.outcome_integrated);
        let outcome_limbo_decided = Rotate::new(&fx.keys.outcome_limbo_decided);
        let outcome_limbo_pending = Rotate::new(&fx.keys.outcome_limbo_pending);
        let outcome_missing = Rotate::new(&fx.keys.outcome_missing);
        let warrantees = Rotate::new(&fx.keys.warrantees);
        let local = &fx.keys.local_author;
        let local_actions = Rotate::new(&fx.keys.action_hashes);
        let mut present_batch: Vec<holo_hash::DhtOpHash> = fx
            .keys
            .op_hashes
            .iter()
            .take(PRESENT_BATCH / 2)
            .cloned()
            .collect();
        present_batch.extend(
            fx.keys
                .limbo_op_hashes
                .iter()
                .take(PRESENT_BATCH / 4)
                .cloned(),
        );
        while present_batch.len() < PRESENT_BATCH {
            let mut raw = vec![0u8; 36];
            raw[0] = present_batch.len() as u8;
            present_batch.push(holo_hash::DhtOpHash::from_raw_36(raw));
        }

        g.throughput(Throughput::Elements(1));
        g.bench_with_input(
            BenchmarkId::new("get_actions_by_prev_hash", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    let h = actions.next();
                    db.get_actions_by_prev_hash(h, h).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new(
                "limbo_chain_ops_pending_sys_validation_with_action",
                fx.size,
            ),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.limbo_chain_ops_pending_sys_validation_with_action(WORKFLOW_LIMIT)
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new(
                "limbo_chain_ops_pending_app_validation_with_action",
                fx.size,
            ),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.limbo_chain_ops_pending_app_validation_with_action(WORKFLOW_LIMIT)
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("limbo_chain_ops_ready_for_integration", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.limbo_chain_ops_ready_for_integration(WORKFLOW_LIMIT)
                        .await
                        .unwrap()
                })
            },
        );
        g.throughput(Throughput::Elements(PRESENT_BATCH as u64));
        g.bench_with_input(
            BenchmarkId::new("op_hashes_present_100", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.op_hashes_present(&present_batch).await.unwrap() })
            },
        );
        g.throughput(Throughput::Elements(1));
        g.bench_with_input(
            BenchmarkId::new("pending_or_valid_warrant_proofs_by_warrantee", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.pending_or_valid_warrant_proofs_by_warrantee(warrantees.next())
                        .await
                        .unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("op_validation_outcome", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.op_validation_outcome(actions.next(), 1).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("op_validation_outcome_integrated", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    let sample = outcome_integrated.next();
                    db.op_validation_outcome(&sample.0, sample.1).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("op_validation_outcome_limbo_decided", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    let sample = outcome_limbo_decided.next();
                    db.op_validation_outcome(&sample.0, sample.1).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("op_validation_outcome_limbo_pending", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    let sample = outcome_limbo_pending.next();
                    db.op_validation_outcome(&sample.0, sample.1).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("op_validation_outcome_missing", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    let sample = outcome_missing.next();
                    db.op_validation_outcome(&sample.0, sample.1).await.unwrap()
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("pending_validation_receipts", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.pending_validation_receipts().await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("get_ops_to_publish", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt)
                    .iter(|| async { db.get_ops_to_publish(local, i64::MAX).await.unwrap() })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("validation_receipts_for_action", fx.size),
            &fx.size,
            |b, _| {
                b.to_async(rt).iter(|| async {
                    db.validation_receipts_for_action(local_actions.next().clone())
                        .await
                        .unwrap()
                })
            },
        );
    }
    g.finish();
}
