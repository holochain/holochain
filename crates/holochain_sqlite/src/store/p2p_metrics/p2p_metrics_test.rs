use crate::prelude::*;
use kitsune_p2p_bin_data::{KitsuneAgent, KitsuneSpace};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::metrics::{MetricRecord, MetricRecordKind};
use rand::Rng;
use std::sync::Arc;

fn rand_space() -> Arc<KitsuneSpace> {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    Arc::new(KitsuneSpace(data))
}

fn rand_agent() -> Arc<KitsuneAgent> {
    let mut rng = rand::thread_rng();

    let mut data = vec![0_u8; 36];
    rng.fill(&mut data[..]);
    Arc::new(KitsuneAgent(data))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_metric_store_sanity() {
    let tmp_dir = tempfile::Builder::new()
        .prefix("p2p_agent_store_gossip_query_sanity")
        .tempdir()
        .unwrap();

    let space = rand_space();

    let db = DbWrite::test(tmp_dir.path(), DbKindP2pMetrics(space.clone())).unwrap();

    db.read_async(move |txn| {
        txn.p2p_log_metrics(vec![
            // -- reachability quotient -- //
            MetricRecord {
                kind: MetricRecordKind::ReachabilityQuotient,
                agent: Some(rand_agent()),
                recorded_at_utc: Timestamp::MIN,
                expires_at_utc: Timestamp::MAX,
                data: serde_json::json!(42.42),
            },
            // -- latency micros -- //
            MetricRecord {
                kind: MetricRecordKind::LatencyMicros,
                agent: Some(rand_agent()),
                recorded_at_utc: Timestamp::MIN,
                expires_at_utc: Timestamp::MAX,
                data: serde_json::json!(42.42),
            },
            // -- agg extrap cov -- //
            MetricRecord {
                kind: MetricRecordKind::AggExtrapCov,
                agent: None,
                recorded_at_utc: Timestamp::MIN,
                expires_at_utc: Timestamp::MAX,
                data: serde_json::json!(42.42),
            },
        ])
    })
    .await
    .unwrap();

    // clean up temp dir
    tmp_dir.close().unwrap();
}
