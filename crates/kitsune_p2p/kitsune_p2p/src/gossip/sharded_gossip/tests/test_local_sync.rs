use maplit::hashset;

use super::common::*;
use super::handler_builder::handler_builder;
use super::*;

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
fn generate_ops_for_overlapping_arcs<'a>(
    entropy: &mut arbitrary::Unstructured<'a>,
    ownership: &[HashSet<Arc<KitsuneAgent>>],
) -> Vec<(
    Arc<KitsuneAgent>,
    ArcInterval,
    Vec<(KitsuneOpHash, TimestampMs)>,
)> {
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

#[tokio::test(flavor = "multi_thread")]
async fn local_sync_scenario() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let agents = agents(3);
    let alice = agents[0].clone();
    let bobbo = agents[1].clone();
    let carol = agents[2].clone();
    let ownership = &[
        hashset![alice.clone()],
        hashset![alice.clone(), bobbo.clone()],
        hashset![bobbo.clone()],
        hashset![bobbo.clone(), carol.clone()],
        hashset![carol.clone()],
        hashset![carol.clone(), alice.clone()],
    ];
    let data = generate_ops_for_overlapping_arcs(&mut u, ownership);
    let mut evt_handler = handler_builder(data).await;

    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let gossip = ShardedGossipLocal::test(GossipType::Recent, evt_sender, Default::default());

    gossip.local_sync().await.unwrap();

    let _cert = Tx2Cert::arbitrary(&mut u);
}
