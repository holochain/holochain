//! Queries for the P2pMetrics store

use super::error::ConductorResult;
use holochain_types::prelude::*;
use kitsune_p2p::event::{MetricDatumKind, MetricQuery, MetricQueryAnswer};

/// Record a p2p metric datum
pub fn put_metric_datum(
    env: EnvWrite,
    agent: AgentPubKey,
    metric: MetricDatumKind,
) -> ConductorResult<()> {
    env.with_commit(|txn| {
        txn.execute(
            sql_p2p_metrics::INSERT,
            named_params! {
                ":agent": agent,

                ":encoded": &record.encoded,

                ":signed_at_ms": &record.signed_at_ms,
                ":expires_at_ms": &record.expires_at_ms,
                ":storage_center_loc": &record.storage_center_loc,

                ":storage_start_1": &record.storage_start_1,
                ":storage_end_1": &record.storage_end_1,
                ":storage_start_2": &record.storage_start_2,
                ":storage_end_2": &record.storage_end_2,
            },
        )
    })?;
    Ok(())
}

/// Query the p2p_metrics database in a variety of ways
pub fn query_metrics(_env: EnvWrite, _query: MetricQuery) -> ConductorResult<MetricQueryAnswer> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holochain_p2p::agent_holo_to_kit;
    use holochain_state::prelude::test_p2p_state_env;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_last_sync() {
        let test_env = test_p2p_state_env();
        let env = test_env.env();
        let agent1 = fixt!(AgentPubKey);
        let agent2 = fixt!(AgentPubKey);
        // Vec of successively later Instants
        let instants: Vec<Instant> = itertools::unfold(Instant::now(), |now| {
            now.checked_add(Duration::from_secs(1))
        })
        .take(5)
        .collect();

        put_metric_datum(
            env.clone(),
            agent1.clone(),
            MetricDatumKind::LastQuickGossip(instants[0].clone()),
        )
        .unwrap();

        put_metric_datum(
            env.clone(),
            agent2.clone(),
            MetricDatumKind::LastQuickGossip(instants[1].clone()),
        )
        .unwrap();

        put_metric_datum(
            env.clone(),
            agent1.clone(),
            MetricDatumKind::LastQuickGossip(instants[2].clone()),
        )
        .unwrap();

        put_metric_datum(
            env.clone(),
            agent1.clone(),
            MetricDatumKind::LastQuickGossip(instants[3].clone()),
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
            MetricQueryAnswer::LastSync(instants[2].clone())
        );
        assert_eq!(
            query_metrics(
                env.clone(),
                MetricQuery::LastSync {
                    agent: Arc::new(agent_holo_to_kit(agent2))
                }
            )
            .unwrap(),
            MetricQueryAnswer::LastSync(instants[3].clone())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_oldest() {
        let test_env = test_p2p_state_env();
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
