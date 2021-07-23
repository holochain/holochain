use super::common::*;
use super::handler_builder::handler_builder;
use super::*;

/// Given a list of ownership requirements, returns
/// - a collection of agents paired arcs defined by the ownership requirements,
/// - and the list of ops which falls under those arcs
///
/// The ownership requirements are defined as so:
/// Each item corresponds to a to-be-created op hash.
/// The set of Agents specifies which Agent is holding that op.
/// Then, op hashes are assigned, in increasing DHT location order, to each set
/// of agents specified.
/// This has the effect of allowing arbitrary overlapping arcs to be defined,
/// backed by real op hash data, without worrying about particular DHT locations
/// (which would have to be searched for).
fn generate_ops_for_overlapping_arcs<'a>(
    entropy: &mut arbitrary::Unstructured<'a>,
    ownership: Vec<HashSet<Arc<KitsuneAgent>>>,
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
    // grow the arcs for each
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
    let agents = generate_ops_for_overlapping_arcs(&mut u, vec![]);
    let evt_handler = handler_builder(agents).await;
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let _gossip = ShardedGossipLocal::test(GossipType::Recent, evt_sender, Default::default());

    let _cert = Tx2Cert::arbitrary(&mut u);

    todo!("write scenario")
}
