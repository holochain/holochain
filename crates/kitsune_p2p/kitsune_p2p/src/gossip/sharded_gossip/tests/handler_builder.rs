use super::common::agent_info;
use super::*;

/// Data which can be used to implement a mock kitsune backend handler.
/// Specifies a list of local agents along with their arc and op hashes held.
pub type MockBackendData = Vec<(
    Arc<KitsuneAgent>,
    ArcInterval,
    Vec<(KitsuneOpHash, TimestampMs)>,
)>;

/// Produces a mock handler which can accomodate multiple agents with different
/// ArcIntervals and different sets of data held.
///
/// Limitations:
/// - Op data returned is completely arbitrary and does NOT hash to the hash it's "stored" under
pub async fn handler_builder(agent_data: MockBackendData) -> MockKitsuneP2pEventHandler {
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

/// Concise representation of data held by various agents in a sharded scenario.
/// See [`generate_ops_for_overlapping_arcs`] for usage detail.
//
// This could have been a Vec<>, but it's nice to be able to use this as a slice.
pub type OwnershipData<const N: usize> = [HashSet<Arc<KitsuneAgent>>; N];

/// Given a list of ownership requirements, returns a list of triples, each
/// item of which consists of:
/// - an agent
/// - its Arc
/// - a list of ops that it holds
///
/// The list of ops is guaranteed to fit within the arc it is matched with.
/// Also, the arcs and ops will overlap as specified by the `ownership` input.
///
/// The ownership requirements are defined as so:
/// - Each item corresponds to a to-be-created op hash.
/// - The set of Agents specifies which Agent is holding that op.
/// - Then, op hashes are assigned, in increasing DHT location order, to each set
///     of agents specified.
///
/// This has the effect of allowing arbitrary overlapping arcs to be defined,
/// backed by real op hash data, without worrying about particular DHT locations
/// (which would have to be searched for).
pub fn generate_ops_for_overlapping_arcs<'a, const N: usize>(
    entropy: &mut arbitrary::Unstructured<'a>,
    ownership: OwnershipData<N>,
) -> MockBackendData {
    let mut arcs: HashMap<Arc<KitsuneAgent>, ((u32, u32), Vec<KitsuneOpHash>)> = HashMap::new();
    // create one op per "ownership" item
    let mut ops: Vec<KitsuneOpHash> = ownership
        .iter()
        .map(|_| KitsuneOpHash::arbitrary(entropy).unwrap())
        .collect();

    // sort ops by location
    ops.sort_by_key(|op| op.get_loc());

    // associate ops with relevant agents, growing arcs at the same time
    for (owners, op) in itertools::zip(ownership.into_iter(), ops.into_iter()) {
        for owner in owners.clone() {
            arcs.entry(owner)
                .and_modify(|((_, hi), ops)| {
                    *hi = op.get_loc();
                    ops.push(op.clone())
                })
                .or_insert(((op.get_loc(), op.get_loc()), vec![op.clone()]));
        }
    }

    // Construct the ArcIntervals, and for now, associate an arbitrary timestamp
    // with each op
    let arcs = arcs
        .into_iter()
        .map(|(agent, ((lo, hi), ops))| {
            let ops = ops.into_iter().map(|op| (op, 1111)).collect();
            (agent, ArcInterval::Bounded(lo, hi), ops)
        })
        .collect();
    arcs
}
