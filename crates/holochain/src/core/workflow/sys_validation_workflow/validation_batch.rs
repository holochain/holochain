use crate::core::workflow::error::WorkflowResult;
use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::core::workflow::sys_validation_workflow::OutcomeSummary;
use futures::future::BoxFuture;
use futures::stream::StreamExt;
use holo_hash::DhtOpHash;
use holochain_state::prelude::Dependency;
use holochain_types::prelude::DhtOpHashed;
use std::time::Instant;

pub const NUM_CONCURRENT_OPS: usize = 50;

pub(super) async fn validate_ops_batch(
    ops: Vec<DhtOpHashed>,
    started_at: Option<Instant>,
    validator_fn: impl Fn(DhtOpHashed) -> BoxFuture<'static, WorkflowResult<(DhtOpHash, Outcome, Dependency)>>
        + Send
        + 'static,
    commit_outcome_batch_fn: impl Fn(
        Vec<WorkflowResult<(DhtOpHash, Outcome, Dependency)>>,
    ) -> BoxFuture<'static, WorkflowResult<OutcomeSummary>>,
) -> WorkflowResult<Vec<OutcomeSummary>> {
    let start_len = ops.len();

    // Process each op
    let iter = ops.into_iter().map(validator_fn);

    // Create a stream of concurrent validation futures.
    // This will run NUM_CONCURRENT_OPS validation futures concurrently and
    // return up to NUM_CONCURRENT_OPS * 100 results.
    let mut iter = futures::stream::iter(iter)
        .buffer_unordered(NUM_CONCURRENT_OPS) // TODO So why sort the ops if we're going to run this through in any order?
        .ready_chunks(NUM_CONCURRENT_OPS * 100);

    let mut summaries = vec![];
    let mut total = 0;
    let mut round_time = started_at.is_some().then(std::time::Instant::now);
    // Pull in a chunk of results.
    while let Some(chunk) = iter.next().await {
        let num_ops: usize = chunk.len();
        tracing::debug!("Committing {} ops", num_ops);
        let summary = commit_outcome_batch_fn(chunk).await?;

        total += summary.accepted;
        if let (Some(start), Some(round_time)) = (started_at, &mut round_time) {
            let round_el = round_time.elapsed();
            *round_time = std::time::Instant::now();
            let avg_ops_ps = total as f64 / start.elapsed().as_micros() as f64 * 1_000_000.0;
            let ops_ps = summary.accepted as f64 / round_el.as_micros() as f64 * 1_000_000.0;
            tracing::info!(
                "Sys validation is saturated. Util {:.2}%. OPS/s avg {:.2}, this round {:.2}",
                (start_len - total) as f64 / NUM_CONCURRENT_OPS as f64 * 100.0,
                avg_ops_ps,
                ops_ps
            );
        }
        tracing::debug!("{} committed, {} awaiting sys dep, {} missing dht dep, {} rejected. {} committed this round", summary.accepted, summary.awaiting, summary.missing, summary.rejected, total);
        summaries.push(summary);
    }

    tracing::debug!("Accepted {} ops", total);

    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::validate_ops_batch;
    use crate::core::workflow::error::WorkflowError;
    use crate::core::workflow::sys_validation_workflow::validation_batch::NUM_CONCURRENT_OPS;
    use crate::core::workflow::sys_validation_workflow::OutcomeSummary;
    use crate::core::workflow::sys_validation_workflow::types::Outcome;
    use fixt::prelude::*;
    use futures::FutureExt;
    use hdk::prelude::Action;
    use hdk::prelude::CreateFixturator;
    use hdk::prelude::SignatureFixturator;
    use holo_hash::fixt::AnyDhtHashFixturator;
    use holochain::prelude::DhtOp;
    use holochain_state::prelude::Dependency;
    use holochain_types::prelude::DhtOpHashed;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn no_ops_to_process() {
        let summaries = validate_ops_batch(
            vec![],
            None,
            |op| async move { Ok((op.hash, Outcome::Accepted, Dependency::Null)) }.boxed(),
            |batch| {
                async move {
                    Ok(OutcomeSummary {
                        accepted: batch.len(),
                        awaiting: 0,
                        missing: 0,
                        rejected: 0,
                    })
                }
                .boxed()
            },
        )
        .await
        .unwrap();

        assert!(summaries.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn single_op_to_process() {
        let summaries = validate_ops_batch(
            vec![test_op()],
            None,
            |op| async move { Ok((op.hash, Outcome::Accepted, Dependency::Null)) }.boxed(),
            |batch| {
                async move {
                    Ok(OutcomeSummary {
                        accepted: batch.len(),
                        awaiting: 0,
                        missing: 0,
                        rejected: 0,
                    })
                }
                .boxed()
            },
        )
        .await
        .unwrap();

        assert_eq!(1, summaries.len());
        assert_eq!(1, summaries[0].accepted);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn respects_limits_when_processing_large_batch() {
        holochain_trace::test_run().unwrap();

        let high_water_mark = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));

        let summaries = validate_ops_batch(
            std::iter::repeat_with(test_op).take(20_505).collect(),
            Some(std::time::Instant::now()),
            {
                let high_water_mark = high_water_mark.clone();
                move |op| {
                    in_flight.fetch_add(1, Ordering::SeqCst);

                    let high_water_mark = high_water_mark.clone();
                    let in_flight = in_flight.clone();
                    async move {
                        tokio::time::sleep(Duration::from_nanos(rand::random::<u8>().into())).await;

                        let num_in_flight = in_flight.fetch_sub(1, Ordering::SeqCst);
                        if num_in_flight > high_water_mark.load(Ordering::SeqCst) {
                            high_water_mark.store(num_in_flight, Ordering::SeqCst);
                        }

                        Ok((op.hash, Outcome::Accepted, Dependency::Null))
                    }
                    .boxed()
                }
            },
            |batch| {
                async move {
                    Ok(OutcomeSummary {
                        accepted: batch.len(),
                        awaiting: 0,
                        missing: 0,
                        rejected: 0,
                    })
                }
                .boxed()
            },
        )
        .await
        .unwrap();

        // Should reach but not exceed this constant.
        assert_eq!(NUM_CONCURRENT_OPS, high_water_mark.load(Ordering::SeqCst));

        // NUM_CONCURRENT_OPS * 100 is the max chunk size, or 5_000 ops. So to process 20_505 we should need 5 chunks.
        assert!(summaries.len() >= 5);
        assert!(summaries.iter().map(|s| s.accepted).max().unwrap() <= NUM_CONCURRENT_OPS * 100);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn permits_validation_errors() {
        let summaries = validate_ops_batch(
            std::iter::repeat_with(test_op).take(30).collect(),
            None,
            |op| {
                async move {
                    if rand::random::<bool>() {
                        Err(WorkflowError::other("test error"))
                    } else {
                        Ok((op.hash, Outcome::Accepted, Dependency::Null))
                    }
                }
                .boxed()
            },
            |batch| {
                async move {
                    Ok(OutcomeSummary {
                        accepted: batch.iter().filter(|r| r.is_ok()).count(),
                        awaiting: 0,
                        missing: 0,
                        rejected: 0,
                    })
                }
                .boxed()
            },
        )
        .await
        .unwrap();

        assert_eq!(1, summaries.len());
        assert!(
            summaries[0].accepted < 30,
            "Expected fewer than 30 ops to have been processed successfully but got {}",
            summaries[0].accepted
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn yields_accounting_on_outcomes() {
        let success_op = test_op();
        let awaiting_dep_op = test_op();
        let missing_dep_op = test_op();
        let rejected_op = test_op();

        let summaries = validate_ops_batch(
            vec![
                success_op.clone(),
                awaiting_dep_op.clone(),
                missing_dep_op.clone(),
                rejected_op.clone(),
            ],
            None,
            move |op| {
                let success_op = success_op.clone();
                let awaiting_dep_op = awaiting_dep_op.clone();
                let missing_dep_op = missing_dep_op.clone();
                let rejected_op = rejected_op.clone();
                async move {
                    if op.hash == success_op.hash {
                        Ok((op.hash, Outcome::Accepted, Dependency::Null))
                    } else if op.hash == awaiting_dep_op.hash {
                        Ok((
                            op.hash,
                            Outcome::AwaitingOpDep(fixt!(AnyDhtHash)),
                            Dependency::Null,
                        ))
                    } else if op.hash == missing_dep_op.hash {
                        Ok((op.hash, Outcome::MissingDhtDep, Dependency::Null))
                        unreachable!("Unexpected op")
                    }
                }
                .boxed()
            },
            |batch| {
                async move {
                    Ok(OutcomeSummary {
                        accepted: batch.iter().count(),
                        awaiting: batch
                            .iter()
                            .filter(|r| match r {
                                Ok((_, Outcome::AwaitingOpDep(_), _)) => true,
                                _ => false,
                            })
                            .count(),
                        missing: batch
                            .iter()
                            .filter(|r| match r {
                                Ok((_, Outcome::MissingDhtDep, _)) => true,
                                _ => false,
                            })
                            .count(),
                        rejected: batch
                            .iter()
                            .filter(|r| match r {
                                Ok((_, Outcome::Rejected, _)) => true,
                                _ => false,
                            })
                            .count(),
                    })
                }
                .boxed()
            },
        )
        .await
        .unwrap();

        assert_eq!(1, summaries.len());
        assert_eq!(4, summaries[0].accepted);
        assert_eq!(1, summaries[0].awaiting);
        assert_eq!(1, summaries[0].missing);
        assert_eq!(1, summaries[0].rejected);
    }

    fn test_op() -> DhtOpHashed {
        let create_action = fixt!(Create);
        let action = Action::Create(create_action);

        DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(fixt!(Signature), action))
    }
}
