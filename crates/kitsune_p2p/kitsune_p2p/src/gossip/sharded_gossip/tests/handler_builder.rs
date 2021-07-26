use super::common::agent_info;
use super::common::spawn_handler;
use super::test_local_sync::three_way_sharded_ownership;
use super::*;

/// Data which represents the agent store of a backend.
/// Specifies a list of agents along with their arc and timestamped op hashes held.
pub type MockAgentPersistence = Vec<(
    Arc<KitsuneAgent>,
    ArcInterval,
    Vec<(KitsuneOpHash, TimestampMs)>,
)>;

/// Build up the functionality of a mock event handler a la carte with these
/// provided methods
pub struct HandlerBuilder(MockKitsuneP2pEventHandler);

impl HandlerBuilder {
    /// Constructor
    pub fn new() -> Self {
        Self(MockKitsuneP2pEventHandler::new())
    }

    /// Make the mock available
    pub fn build(self) -> MockKitsuneP2pEventHandler {
        self.0
    }

    /// Mocks gossip to do nothing
    pub fn with_noop_gossip(mut self, agent_data: MockAgentPersistence) -> Self {
        self.0
            .expect_handle_gossip()
            .returning(|_, _, _, _, _| Ok(async { Ok(()) }.boxed().into()));

        self
    }

    /// Implements the agent persistence methods (fetches and gets) to act as if
    /// it is backed by a data store with the provided agent data.
    ///
    /// Limitations:
    /// - Op data returned is completely arbitrary and does NOT hash to the hash it's "stored" under
    pub fn with_agent_persistence(mut self, agent_data: MockAgentPersistence) -> Self {
        let agents_only: Vec<_> = agent_data.iter().map(|(a, _, _)| a.clone()).collect();
        let agents_arcs: Vec<_> = agent_data
            .iter()
            .map(|(agent, arc, _)| (agent.clone(), arc.clone()))
            .collect();

        self.0.expect_handle_query_agent_info_signed().returning({
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

        self.0.expect_handle_get_agent_info_signed().returning({
            let agents = agents_only.clone();
            move |input| {
                let agents = agents.clone();
                let agent = agents.iter().find(|a| **a == input.agent).unwrap().clone();
                Ok(async move { Ok(Some(agent_info(agent).await)) }
                    .boxed()
                    .into())
            }
        });

        self.0.expect_handle_query_gossip_agents().returning({
            move |_| {
                let agents_arcs = agents_arcs.clone();
                Ok(async move { Ok(agents_arcs) }.boxed().into())
            }
        });

        self.0
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

        self.0
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

        self
    }
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
///
/// See the test below for a thorough example.
pub fn generate_ops_for_overlapping_arcs<'a, const N: usize>(
    entropy: &mut arbitrary::Unstructured<'a>,
    ownership: OwnershipData<N>,
) -> MockAgentPersistence {
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

/// Test that the above functions work as expected in one specific case:
/// Out of 6 ops total, all 3 agents hold 3 ops each.
#[tokio::test(flavor = "multi_thread")]
async fn test_three_way_sharded_ownership() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let space = Arc::new(KitsuneSpace::arbitrary(&mut u).unwrap());
    let (agents, ownership) = three_way_sharded_ownership();
    let persistence = generate_ops_for_overlapping_arcs(&mut u, ownership);
    let agent_arcs: Vec<_> = persistence
        .iter()
        .map(|(agent, arc, _)| (agent.clone(), arc.clone()))
        .collect();

    let evt_handler = HandlerBuilder::new()
        .with_agent_persistence(persistence)
        .build();
    let (evt_sender, _) = spawn_handler(evt_handler).await;

    // Closure to reduce boilerplate
    let get_op_hashes = |a: usize| {
        let evt_sender = evt_sender.clone();
        let space = space.clone();
        async move {
            store::all_op_hashes_within_arcset(
                &evt_sender,
                &space,
                // Only look at one agent at a time
                &agent_arcs[a..a + 1],
                &DhtArcSet::Full,
                full_time_window(),
                usize::MAX,
            )
            .await
            .unwrap()
            .unwrap()
            .0
        }
    };

    // - All arcs cover 3 hashes
    let op_hashes_0 = (get_op_hashes.clone())(0).await;
    let op_hashes_1 = (get_op_hashes.clone())(1).await;
    let op_hashes_2 = (get_op_hashes.clone())(2).await;
    assert_eq!(
        (op_hashes_0.len(), op_hashes_1.len(), op_hashes_2.len()),
        (3, 3, 3)
    );

    // - All hashes point to an actual retrievable op
    let ops_0 = store::fetch_ops(
        &evt_sender,
        &space,
        agents.iter().skip(0).take(1),
        op_hashes_0,
    )
    .await
    .unwrap();
    let ops_1 = store::fetch_ops(
        &evt_sender,
        &space,
        agents.iter().skip(1).take(1),
        op_hashes_1,
    )
    .await
    .unwrap();
    let ops_2 = store::fetch_ops(
        &evt_sender,
        &space,
        agents.iter().skip(2).take(1),
        op_hashes_2,
    )
    .await
    .unwrap();
    assert_eq!((ops_0.len(), ops_1.len(), ops_2.len()), (3, 3, 3));

    // - There are only 6 distinct ops
    let mut all_ops = HashSet::new();
    all_ops.extend(ops_0.into_iter());
    all_ops.extend(ops_1.into_iter());
    all_ops.extend(ops_2.into_iter());
    assert_eq!(all_ops.len(), 6);
}
