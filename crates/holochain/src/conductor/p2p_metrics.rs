//! Queries for the P2pMetrics store
// TODO [ B-04249 ] move this to the combined holochain_sqlite crate once
// consolidated with holochain_state

use super::error::ConductorResult;
use holochain_p2p::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use kitsune_p2p::event::{MetricKind, MetricQuery, MetricQueryAnswer};
use std::time::SystemTime;

/// Record a p2p metric datum
pub async fn put_metric_datum(
    env: DbWrite<DbKindP2pMetrics>,
    agent: AgentPubKey,
    metric: MetricKind,
    timestamp: SystemTime,
) -> ConductorResult<()> {
    env.async_commit(move |txn| {
        holochain_sqlite::db::put_metric_datum(txn, agent.to_kitsune(), metric, timestamp)
    })
    .await?;
    Ok(())
}

/// Query the p2p_metrics database in a variety of ways
pub async fn query_metrics(
    env: DbWrite<DbKindP2pMetrics>,
    query: MetricQuery,
) -> ConductorResult<MetricQueryAnswer> {
    Ok(env
        .conn()?
        .with_reader(move |mut txn| holochain_sqlite::db::query_metrics(&mut txn, query))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holochain_p2p::agent_holo_to_kit;
    use holochain_sqlite::db::{time_from_micros, time_to_micros};
    use holochain_state::prelude::test_p2p_metrics_env;
    use std::{sync::Arc, time::Duration};

    /// Return an iterator of moments, each one later than the last by 1 second,
    /// and at a granularity of microseconds
    fn moments() -> impl Iterator<Item = SystemTime> {
        let initial = time_from_micros(time_to_micros(SystemTime::now()).unwrap()).unwrap();
        itertools::unfold(initial, |t| {
            t.checked_add(Duration::from_secs(1)).map(|next| {
                *t = next;
                next
            })
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_last_sync() {
        use MetricKind::*;

        let test_env = test_p2p_metrics_env();
        let env = test_env.env();
        let agent1 = fixt!(AgentPubKey);
        let agent2 = fixt!(AgentPubKey);
        let agent3 = fixt!(AgentPubKey);

        let ms: Vec<SystemTime> = moments().take(6).collect();

        // insert relevant data
        put_metric_datum(env.clone(), agent1.clone(), QuickGossip, ms[0].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), QuickGossip, ms[1].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent1.clone(), QuickGossip, ms[2].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), QuickGossip, ms[3].clone())
            .await
            .unwrap();

        // other metrics do not affect this query even if they're more recent
        put_metric_datum(env.clone(), agent1.clone(), SlowGossip, ms[4].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), SlowGossip, ms[5].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent1.clone(), ConnectError, ms[4].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), ConnectError, ms[5].clone())
            .await
            .unwrap();
        // more unrelated noise
        put_metric_datum(env.clone(), agent3.clone(), QuickGossip, ms[5].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent3.clone(), SlowGossip, ms[5].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent3.clone(), ConnectError, ms[5].clone())
            .await
            .unwrap();

        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::LastSync {
                    agent: Arc::new(agent_holo_to_kit(agent1))
                }
            )
            .await
            .unwrap(),
            MetricQueryAnswer::LastSync(Some(ms[2].clone()))
        );

        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::LastSync {
                    agent: Arc::new(agent_holo_to_kit(agent2))
                }
            )
            .await
            .unwrap(),
            MetricQueryAnswer::LastSync(Some(ms[3].clone()))
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_oldest() {
        use MetricKind::*;

        let test_env = test_p2p_metrics_env();
        let env = test_env.env();
        let agent1 = fixt!(AgentPubKey);
        let agent2 = fixt!(AgentPubKey);
        let ms: Vec<SystemTime> = moments().take(8).collect();

        // insert relevant data
        put_metric_datum(env.clone(), agent1.clone(), SlowGossip, ms[0].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), SlowGossip, ms[1].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent1.clone(), SlowGossip, ms[2].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), SlowGossip, ms[3].clone())
            .await
            .unwrap();
        // we're reusing some of the same moments here for convenience, but
        // there is no significance in that fact.
        put_metric_datum(env.clone(), agent1.clone(), ConnectError, ms[0].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), ConnectError, ms[2].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), ConnectError, ms[4].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent1.clone(), ConnectError, ms[6].clone())
            .await
            .unwrap();

        // a little noise
        put_metric_datum(env.clone(), agent1.clone(), QuickGossip, ms[0].clone())
            .await
            .unwrap();
        put_metric_datum(env.clone(), agent2.clone(), QuickGossip, ms[0].clone())
            .await
            .unwrap();

        // - agent 1 has the oldest latest slow gossip time, but the most recent
        //   connection error, so with this threshold, agent 2 should be returned.
        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::Oldest {
                    last_connect_error_threshold: ms[5].clone()
                }
            )
            .await
            .unwrap(),
            MetricQueryAnswer::Oldest(Some(agent2.to_kitsune()))
        );

        // - with a more recent threshold, agent 1 should be returned, since
        //   it is not filtered out
        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::Oldest {
                    last_connect_error_threshold: ms[7].clone()
                }
            )
            .await
            .unwrap(),
            MetricQueryAnswer::Oldest(Some(agent1.to_kitsune()))
        );
    }
}
