//! Queries for the P2pMetrics store
// TODO [ B-04249 ] move this to the combined holochain_sqlite crate once
// consolidated with holochain_state

use super::error::ConductorResult;
use holochain_p2p::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use kitsune_p2p::event::{MetricDatumKind, MetricQuery, MetricQueryAnswer};
use std::time::SystemTime;

/// Record a p2p metric datum
pub fn put_metric_datum(
    env: EnvWrite,
    agent: AgentPubKey,
    metric: MetricDatumKind,
    timestamp: SystemTime,
) -> ConductorResult<()> {
    env.conn()?.with_commit(|txn| {
        holochain_sqlite::db::put_metric_datum(txn, agent.to_kitsune(), metric, timestamp)
    })?;
    Ok(())
}

/// Query the p2p_metrics database in a variety of ways
pub fn query_metrics(env: EnvWrite, query: MetricQuery) -> ConductorResult<MetricQueryAnswer> {
    Ok(env
        .conn()?
        .with_commit(|txn| holochain_sqlite::db::query_metrics(txn, query))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holochain_p2p::agent_holo_to_kit;
    use holochain_state::prelude::test_p2p_metrics_env;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    fn moments() -> impl Iterator<Item = SystemTime> {
        itertools::unfold(SystemTime::now(), |now| {
            now.checked_add(Duration::from_secs(1)).map(|next| {
                *now = next;
                next
            })
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_last_sync() {
        let test_env = test_p2p_metrics_env();
        let env = test_env.env();
        let agent1 = fixt!(AgentPubKey);
        let agent2 = fixt!(AgentPubKey);
        // Vec of successively later Instants
        let moments: Vec<SystemTime> = moments().take(5).collect();

        dbg!(&moments);

        put_metric_datum(
            env.clone(),
            agent1.clone(),
            MetricDatumKind::LastQuickGossip,
            moments[0].clone(),
        )
        .unwrap();

        put_metric_datum(
            env.clone(),
            agent2.clone(),
            MetricDatumKind::LastQuickGossip,
            moments[1].clone(),
        )
        .unwrap();

        put_metric_datum(
            env.clone(),
            agent1.clone(),
            MetricDatumKind::LastQuickGossip,
            moments[2].clone(),
        )
        .unwrap();

        put_metric_datum(
            env.clone(),
            agent2.clone(),
            MetricDatumKind::LastQuickGossip,
            moments[3].clone(),
        )
        .unwrap();

        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::LastSync {
                    agent: Arc::new(agent_holo_to_kit(agent1))
                }
            )
            .unwrap(),
            MetricQueryAnswer::LastSync(moments[2].clone())
        );
        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::LastSync {
                    agent: Arc::new(agent_holo_to_kit(agent2))
                }
            )
            .unwrap(),
            MetricQueryAnswer::LastSync(moments[3].clone())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_oldest() {
        let test_env = test_p2p_metrics_env();
        let env = test_env.env();
        let agent1 = fixt!(AgentPubKey);
        let agent2 = fixt!(AgentPubKey);
        let instants: Vec<Instant> = itertools::unfold(Instant::now(), |now| {
            now.checked_add(Duration::from_secs(1))
        })
        .take(5)
        .collect();

        todo!();
    }
}
