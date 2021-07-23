use super::common::agent_info;
use super::*;

/// Produces a mock handler which can accomodate multiple agents with different
/// ArcIntervals and different sets of data held.
///
/// Does NOT handle data timestamps, so time windows are ignored.
pub async fn handler_builder(
    agent_data: Vec<(
        Arc<KitsuneAgent>,
        ArcInterval,
        Vec<(KitsuneOpHash, TimestampMs)>,
    )>,
) -> MockKitsuneP2pEventHandler {
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    let agents_only: Vec<_> = agent_data.iter().map(|(a, _, _)| a.clone()).collect();
    let agents_arcs: Vec<_> = agent_data
        .iter()
        .map(|(agent, arc, _)| (agent.clone(), arc.clone()))
        .collect();
    let _agents_ops: Vec<_> = agent_data
        .iter()
        .map(|(agent, _, op)| (agent.clone(), op.clone()))
        .collect();
    evt_handler
        .expect_handle_query_agent_info_signed()
        .returning({
            let agents = agents_only.clone();
            move |_| {
                let agents = agents.clone();
                Ok(async move {
                    let mut infos = Vec::new();
                    for agent in agents {
                        infos.push(agent_info(agent).await);
                    }
                    Ok(infos)
                }
                .boxed()
                .into())
            }
        });
    evt_handler
        .expect_handle_get_agent_info_signed()
        .returning({
            let agents = agents_only.clone();
            move |input| {
                let agents = agents.clone();
                let agent = agents.iter().find(|a| **a == input.agent).unwrap().clone();
                Ok(async move { Ok(Some(agent_info(agent).await)) }
                    .boxed()
                    .into())
            }
        });
    evt_handler.expect_handle_query_gossip_agents().returning({
        move |_| {
            let agents_arcs = agents_arcs.clone();
            Ok(async move { Ok(agents_arcs) }.boxed().into())
        }
    });

    evt_handler
        .expect_handle_fetch_op_hashes_for_constraints()
        .returning(move |arg: FetchOpHashesForConstraintsEvt| {
            // Return ops for agent, correctly filtered by arc but not by time window
            let FetchOpHashesForConstraintsEvt {
                space: _,
                agents,
                window_ms,
                max_ops,
            } = arg;

            let agent_arcsets: HashMap<_, _> = agents.into_iter().collect();

            let mut ops: Vec<&(KitsuneOpHash, TimestampMs)> = agent_data
                .iter()
                .filter_map(|(agent, _, ops)| {
                    if let Some(arcset) = agent_arcsets.get(agent) {
                        Some(
                            ops.into_iter()
                                .filter(|(op, time)| {
                                    window_ms.contains(time) && arcset.contains(op.get_loc())
                                })
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    }
                })
                .flatten()
                .collect();

            ops.sort_by_key(|(_, time)| time);
            ops.dedup();
            let result: Option<(Vec<Arc<KitsuneOpHash>>, TimeWindowMs)> =
                if let (Some((_, first)), Some((_, last))) = (ops.first(), ops.last()) {
                    let ops = ops
                        .into_iter()
                        .map(|(op, _)| Arc::new(op.clone()))
                        .take(max_ops)
                        .collect();
                    Some((ops, *first..*last))
                } else {
                    None
                };
            Ok(async { Ok(result) }.boxed().into())
        });

    evt_handler
        .expect_handle_fetch_op_hash_data()
        .returning(|arg: FetchOpHashDataEvt| {
            // Return dummy data for each op
            let FetchOpHashDataEvt {
                space: _,
                agents: _,
                op_hashes,
            } = arg;
            Ok(async {
                Ok(itertools::zip(op_hashes.into_iter(), std::iter::repeat(vec![0])).collect())
            }
            .boxed()
            .into())
        });

    evt_handler
        .expect_handle_gossip()
        .returning(|_, _, _, _, _| Ok(async { Ok(()) }.boxed().into()));
    evt_handler
}
