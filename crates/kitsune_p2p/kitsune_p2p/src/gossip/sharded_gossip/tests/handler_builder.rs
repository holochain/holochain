use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::dht_arc::DhtArc;

use crate::gossip::sharded_gossip::tests::common::dangerous_fake_agent_info_with_arc;

use super::common::agent_info;
use super::common::spawn_handler;
use super::test_local_sync::three_way_sharded_ownership;
use super::*;

/// Data which represents the agent store of a backend.
/// Specifies a list of agents along with their arc and timestamped op hashes held.
pub type MockAgentPersistence = Vec<(AgentInfoSigned, Vec<(KitsuneOpHash, Timestamp)>)>;

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
    #[allow(dead_code, unused_variables)]
    pub fn with_noop_gossip(mut self, agent_data: MockAgentPersistence) -> Self {
        self.0
            .expect_handle_gossip()
            .returning(|_, _, _| Ok(async { Ok(()) }.boxed().into()));

        self
    }

    /// Implements the agent persistence methods (fetches and gets) to act as if
    /// it is backed by a data store with the provided agent data.
    ///
    /// Limitations/Discrepancies:
    /// - Op data returned is completely arbitrary and does NOT hash to the hash it's "stored" under
    /// - The agent location will NOT be centered on their DhtArc
    pub fn with_agent_persistence(mut self, agent_data: MockAgentPersistence) -> Self {
        let info_only: Vec<_> = agent_data.iter().map(|(info, _)| info.clone()).collect();
        let agents_only: Vec<_> = info_only.iter().map(|info| info.agent.clone()).collect();
        let agents_arcs: Vec<_> = agent_data
            .iter()
            .map(|(info, _)| (info.agent.clone(), info.storage_arc.interval()))
            .collect();

        self.0.expect_handle_query_agent_info_signed().returning({
            let info_only = info_only.clone();
            move |_| {
                let info_only = info_only.clone();
                Ok(async move { Ok(info_only) }.boxed().into())
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
            .expect_handle_query_op_hashes()
            .returning(move |arg: QueryOpHashesEvt| {
                // Return ops for agent, correctly filtered by arc but not by time window
                let QueryOpHashesEvt {
                    space: _,
                    agents,
                    window,
                    max_ops,
                    include_limbo: _,
                } = arg;

                let agent_arcsets: HashMap<_, _> = agents.into_iter().collect();

                let mut ops: Vec<&(KitsuneOpHash, Timestamp)> = agent_data
                    .iter()
                    .filter_map(|(info, ops)| {
                        if let Some(arcset) = agent_arcsets.get(&info.agent) {
                            Some(
                                ops.into_iter()
                                    .filter(|(op, time)| {
                                        window.contains(time) && arcset.contains(op.get_loc())
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
                let result: Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)> =
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
            .expect_handle_fetch_op_data()
            .returning(|arg: FetchOpDataEvt| {
                // Return dummy data for each op
                let FetchOpDataEvt {
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

/// Concise representation of data held by various agents in a sharded scenario,
/// without having to refer to explicit op hashes or locations.
///
/// This type is intended to be used to easily define arbitrary sharded network scenarios,
/// to test various cases of local sync and gossip. It's expected that we'll eventually have a
/// small library of such scenarios, defined in terms of this type.
///
/// See [`generate_ops_for_overlapping_arcs`] for usage detail.
pub struct OwnershipData {
    /// Total number of op hashes to be generated
    total_ops: usize,
    /// Declares arcs and ownership in terms of indices into a vec of generated op hashes.
    agents: Vec<OwnershipDataAgent>,
}

impl OwnershipData {
    /// Construct `OwnershipData` from a more compact "untagged" format using
    /// tuples instead of structs. This is intended to be the canonical constructor.
    pub fn from_compact(total_ops: usize, v: Vec<OwnershipDataAgentCompact>) -> Self {
        Self {
            total_ops,
            agents: v
                .into_iter()
                .map(|(agent, arc_indices, hash_indices)| OwnershipDataAgent {
                    agent,
                    arc_indices,
                    hash_indices,
                })
                .collect(),
        }
    }
}

/// Declares arcs and ownership in terms of indices into a vec of generated op hashes.
pub struct OwnershipDataAgent {
    /// The agent in question
    agent: Arc<KitsuneAgent>,
    /// The start and end indices of the arc for this agent
    arc_indices: (usize, usize),
    /// The indices of ops to consider as owned
    hash_indices: Vec<usize>,
}

/// Same as [`OwnershipDataAgent`], but using a tuple instead of a struct.
/// It's just more compact.
pub type OwnershipDataAgentCompact = (Arc<KitsuneAgent>, (usize, usize), Vec<usize>);

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
pub fn mock_agent_persistence<'a>(
    entropy: &mut arbitrary::Unstructured<'a>,
    ownership: OwnershipData,
) -> (MockAgentPersistence, Vec<KitsuneOpHash>) {
    // create one op per "ownership" item
    let mut hashes: Vec<KitsuneOpHash> = (0..ownership.total_ops)
        .map(|_| KitsuneOpHash::arbitrary(entropy).unwrap())
        .collect();

    // sort hashes by location
    hashes.sort_by_key(|h| h.get_loc());

    // expand the indices provided in the input to actual op hashes and locations,
    // per the gerated ops
    let persistence = ownership
        .agents
        .iter()
        .map(|data| {
            let OwnershipDataAgent {
                agent,
                arc_indices: (arc_idx_lo, arc_idx_hi),
                hash_indices,
            } = data;
            let arc = DhtArc::from_interval(
                ArcInterval::new(hashes[*arc_idx_lo].get_loc(), hashes[*arc_idx_hi].get_loc())
                    .quantized(),
            );
            let hashes = hash_indices
                .into_iter()
                // TODO: `1111` is an arbitrary timestamp placeholder
                .map(|i| (hashes[*i].clone(), Timestamp::from_micros(1111)))
                .collect();
            (
                dangerous_fake_agent_info_with_arc(
                    Arc::new(fixt!(KitsuneSpace)),
                    agent.to_owned(),
                    arc,
                ),
                hashes,
            )
        })
        .collect();
    (persistence, hashes)
}

/// Given some mock persistence data, calculate the diff for each agent, i.e.
/// the ops that would be sent via local sync given the current state.
pub fn calculate_missing_ops(
    data: &MockAgentPersistence,
) -> Vec<(Arc<KitsuneAgent>, Vec<KitsuneOpHash>)> {
    let all_hashes: HashSet<_> = data
        .iter()
        .flat_map(|(_, hs)| hs.iter().map(|(h, _)| h))
        .collect();
    data.iter()
        .map(|(info, hs)| {
            let owned: HashSet<&KitsuneOpHash> = hs.iter().map(|(h, _)| h).collect();
            let (agent, arc) = info.to_agent_arc();
            (
                agent.clone(),
                all_hashes
                    .difference(&owned)
                    .filter(|h| arc.contains(h.get_loc()))
                    .map(|s| s.to_owned().to_owned())
                    .collect(),
            )
        })
        .collect()
}

/// Test that the above functions work as expected in one specific case:
/// Out of 6 ops total, all 3 agents hold 3 ops each.
#[tokio::test(flavor = "multi_thread")]
async fn test_three_way_sharded_ownership() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let space = Arc::new(KitsuneSpace::arbitrary(&mut u).unwrap());
    let (agents, ownership) = three_way_sharded_ownership();
    let (persistence, hashes) = mock_agent_persistence(&mut u, ownership);
    let agent_arcs: Vec<_> = persistence
        .iter()
        .map(|(info, _)| (info.agent.clone(), info.storage_arc.interval()))
        .collect();

    let hold_counts: Vec<_> = agent_arcs
        .iter()
        .map(|(_, arc)| hashes.iter().filter(|h| arc.contains(h.get_loc())).count())
        .collect();

    assert_eq!(hold_counts, vec![3, 3, 3]);

    // - Check that the agents are missing certain hashes (by design)
    assert_eq!(
        calculate_missing_ops(&persistence),
        vec![
            (agents[0].clone(), vec![hashes[1].clone()]),
            (agents[1].clone(), vec![hashes[3].clone()]),
            (agents[2].clone(), vec![hashes[5].clone()]),
        ]
    );

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
                full_time_range(),
                usize::MAX,
                false,
            )
            .await
            .unwrap()
            .unwrap()
            .0
        }
    };

    // - All arcs cover 3 hashes, but each agent only holds 2 of those
    let op_hashes_0 = (get_op_hashes.clone())(0).await;
    let op_hashes_1 = (get_op_hashes.clone())(1).await;
    let op_hashes_2 = (get_op_hashes.clone())(2).await;
    assert_eq!(
        (op_hashes_0.len(), op_hashes_1.len(), op_hashes_2.len()),
        (2, 2, 2)
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
    assert_eq!((ops_0.len(), ops_1.len(), ops_2.len()), (2, 2, 2));

    // - There are only 6 distinct ops
    let mut all_ops = HashSet::new();
    all_ops.extend(ops_0.into_iter());
    all_ops.extend(ops_1.into_iter());
    all_ops.extend(ops_2.into_iter());
    assert_eq!(all_ops.len(), 6);
}
