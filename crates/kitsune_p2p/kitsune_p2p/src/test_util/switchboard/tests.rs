use std::time::Duration;

use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::{
    config::KitsuneP2pTuningParams, dht::spacetime::Topology, dht_arc::loc8::Loc8,
};
use rand::Rng;

use crate::{
    gossip::sharded_gossip::GossipType, test_util::switchboard::switchboard_state::SwitchboardAgent,
};

use super::super::switchboard_state::Switchboard;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread")]
async fn fullsync_3way_recent() {
    // observability::test_run().ok();
    let topo = Topology::standard_epoch_full();
    let sb = Switchboard::new(topo.clone(), GossipType::Recent);

    let [n1, n2, n3] = sb.add_nodes(tuning_params()).await;

    let a1 = SwitchboardAgent::full(1);
    let a2 = SwitchboardAgent::full(2);
    let a3 = SwitchboardAgent::full(3);

    sb.share(|sb| {
        sb.add_local_agent(&n1, &a1);
        sb.add_local_agent(&n2, &a2);
        sb.add_local_agent(&n3, &a3);

        sb.add_ops_now(&n1, true, [10, 20, 30]);
        sb.add_ops_now(&n2, true, [-10, -20, -30]);
        sb.add_ops_now(&n3, true, [-15, 15]);

        // we wouldn't expect this op to be gossiped, since it's from 50+ years ago
        // and hardly "recent"
        sb.add_ops_timed(&n3, true, [(40, Timestamp::from_micros(1))]);

        sb.exchange_all_peer_info();

        // Ensure that the initial conditions are set up properly
        assert_eq!(sb.get_ops_loc8(&n1), Loc8::set([10, 20, 30]));
        assert_eq!(sb.get_ops_loc8(&n2), Loc8::set([-30, -20, -10]));
        assert_eq!(sb.get_ops_loc8(&n3), Loc8::set([-15, 15, 40]));
    });

    // let gossip do its thing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let most = Loc8::set([-30, -20, -15, -10, 10, 15, 20, 30]);
    let mut all = most.clone();
    all.extend(Loc8::set([40]));

    sb.share(|sb| {
        assert_eq!(sb.get_ops_loc8(&n1), most);
        assert_eq!(sb.get_ops_loc8(&n2), most);
        assert_eq!(sb.get_ops_loc8(&n3), all);
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn sharded_3way_recent() {
    observability::test_run().ok();
    let topo = Topology::standard_epoch_full();
    let sb = Switchboard::new(topo.clone(), GossipType::Recent);

    let [n1, n2, n3] = sb.add_nodes(tuning_params()).await;

    let a1 = SwitchboardAgent::from_bounds(-30, 90);
    let a2 = SwitchboardAgent::from_bounds(-90, 30);
    let a3 = SwitchboardAgent::from_bounds(60, -60);

    sb.share(|sb| {
        sb.add_local_agent(&n1, &a1);
        sb.add_local_agent(&n2, &a2);
        sb.add_local_agent(&n3, &a3);

        sb.add_ops_now(&n1, true, [10, 20, 30, 40, 50, 60, 70, 80]);
        sb.add_ops_now(&n2, true, [-10, -20, -30, -40, -50, -60, -70, -80]);
        sb.add_ops_now(&n3, true, [90, 120, -120, -90]);

        sb.print_ascii_arcs(64, true);

        sb.exchange_all_peer_info();
    });

    // let gossip do its thing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    sb.share(|sb| {
        sb.print_ascii_arcs(64, true);
        assert_eq!(
            (
                sb.get_ops_loc8(&n1),
                sb.get_ops_loc8(&n2),
                sb.get_ops_loc8(&n3)
            ),
            (
                Loc8::set([-30, -20, -10, 10, 20, 30, 40, 50, 60, 70, 80, 90]),
                Loc8::set([-90, -80, -70, -60, -50, -40, -30, -20, -10, 10, 20, 30]),
                Loc8::set([-120, -90, -80, -70, -60, 60, 70, 80, 90, 120]),
            )
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn transitive_peer_gossip() {
    observability::test_run().ok();
    let topo = Topology::standard_epoch_full();
    let sb = Switchboard::new(topo.clone(), GossipType::Recent);

    let [n1, n2, n3, n4] = sb.add_nodes(tuning_params()).await;

    let a1 = SwitchboardAgent::from_start_and_len(&topo, 0, 128);
    let a2 = SwitchboardAgent::from_start_and_len(&topo, 64, 128);
    let a3 = SwitchboardAgent::from_start_and_len(&topo, 128, 128);
    let a4 = SwitchboardAgent::from_start_and_len(&topo, 192, 128);

    sb.share(|sb| {
        sb.add_local_agent(&n1, &a1);
        sb.add_local_agent(&n2, &a2);
        sb.add_local_agent(&n3, &a3);
        sb.add_local_agent(&n4, &a4);

        // 1 -> 2 -> 3 -> 4
        // (but 4 does not know about 1 and relies on transitive gossip)
        sb.inject_peer_info(&n1, [&a2]);
        sb.inject_peer_info(&n2, [&a3]);
        sb.inject_peer_info(&n3, [&a4]);

        sb.print_peer_lists();
    });

    // let gossip do its thing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut agent_locs: Vec<_> = vec![a1.clone(), a2.clone(), a3.clone(), a4.clone()]
        .into_iter()
        .map(|a| a.loc)
        .collect();
    agent_locs.sort();

    sb.share(|sb| {
        sb.print_peer_lists();

        // All agent info is shared except perhaps for 4 knowing about 1.
        assert_eq!(
            (&sb.all_peers(&n1), &sb.all_peers(&n2), &sb.all_peers(&n3),),
            (&agent_locs, &agent_locs, &agent_locs,)
        );

        // Once 3 integrates a new op, it will trigger initialize with 4,
        // letting 4 know about 1.
        sb.add_ops_now(&n3, true, [11]);
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    sb.share(|sb| {
        sb.print_peer_lists();
        sb.print_ascii_arcs(32, false);
        assert_eq!(
            (
                &sb.all_peers(&n1),
                &sb.all_peers(&n2),
                &sb.all_peers(&n3),
                &sb.all_peers(&n4)
            ),
            (&agent_locs, &agent_locs, &agent_locs, &agent_locs)
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn sharded_4way_recent() {
    observability::test_run().ok();

    let topo = Topology::standard_epoch_full();
    let sb = Switchboard::new(topo.clone(), GossipType::Recent);

    let [n1, n2, n3, n4] = sb.add_nodes(tuning_params()).await;

    let a1 = SwitchboardAgent::from_start_and_len(&topo, 0, 128);
    let a2 = SwitchboardAgent::from_start_and_len(&topo, 64, 128);
    let a3 = SwitchboardAgent::from_start_and_len(&topo, 128, 128);
    let a4 = SwitchboardAgent::from_start_and_len(&topo, 192, 128);

    let ops: Vec<_> = (0..256).step_by(8).map(|u| Loc8::from(u)).collect();

    sb.share(|sb| {
        sb.add_local_agent(&n1, &a1);
        sb.add_local_agent(&n2, &a2);
        sb.add_local_agent(&n3, &a3);
        sb.add_local_agent(&n4, &a4);

        sb.add_ops_now(&n1, true, ops[0..8].to_vec());
        sb.add_ops_now(&n2, true, ops[8..16].to_vec());
        sb.add_ops_now(&n3, true, ops[16..24].to_vec());
        sb.add_ops_now(&n4, true, ops[24..32].to_vec());

        assert_eq!(
            (
                sb.get_ops_loc8(&n1),
                sb.get_ops_loc8(&n2),
                sb.get_ops_loc8(&n3),
                sb.get_ops_loc8(&n4),
            ),
            (
                Loc8::set(ops[0..8].to_vec()),
                Loc8::set(ops[8..16].to_vec()),
                Loc8::set(ops[16..24].to_vec()),
                Loc8::set(ops[24..32].to_vec()),
            )
        );

        sb.inject_peer_info(&n1, [&a2]);
        sb.inject_peer_info(&n2, [&a3]);
        sb.inject_peer_info(&n3, [&a4]);
        sb.inject_peer_info(&n4, [&a1]);

        sb.print_ascii_arcs(64, true);
    });

    // let gossip do its thing
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    sb.share(|sb| {
        sb.print_ascii_arcs(64, true);

        let mut agent_locs: Vec<_> = vec![a1, a2, a3, a4].into_iter().map(|a| a.loc).collect();
        agent_locs.sort();

        assert_eq!(
            (
                &sb.all_peers(&n1),
                &sb.all_peers(&n2),
                &sb.all_peers(&n3),
                &sb.all_peers(&n4)
            ),
            (&agent_locs, &agent_locs, &agent_locs, &agent_locs)
        );

        assert_eq!(
            (
                sb.get_ops_loc8(&n1),
                sb.get_ops_loc8(&n2),
                sb.get_ops_loc8(&n3),
                sb.get_ops_loc8(&n4),
            ),
            (
                Loc8::set(ops[0..=16].to_vec()),
                Loc8::set(ops[8..=24].to_vec()),
                Loc8::set(ops[16..32].to_vec().into_iter().chain([ops[0]])),
                Loc8::set(ops[24..32].to_vec().into_iter().chain(ops[0..=8].to_vec())),
            )
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn sharded_4way_historical() {
    observability::test_run().ok();

    let now = Timestamp::now().as_micros();
    // 1 year ago
    let then = now - 1_000_000 * 60 * 60 * 24 * 365;
    let topo = Topology::standard(Timestamp::from_micros(then), Duration::ZERO);
    let sb = Switchboard::new(topo.clone(), GossipType::Historical);

    let [n1, n2, n3, n4] = sb.add_nodes(tuning_params()).await;

    let a1 = SwitchboardAgent::from_start_and_len(&topo, 0, 128);
    let a2 = SwitchboardAgent::from_start_and_len(&topo, 64, 128);
    let a3 = SwitchboardAgent::from_start_and_len(&topo, 128, 128);
    let a4 = SwitchboardAgent::from_start_and_len(&topo, 192, 128);
    let ops_only: Vec<_> = (0..256).step_by(8).map(|u| Loc8::from(u)).collect();
    let ops_timed: Vec<_> = ops_only
        .clone()
        .into_iter()
        .map(|loc| {
            (
                loc,
                Timestamp::from_micros(rand::thread_rng().gen_range(then..now)),
            )
        })
        .collect();

    sb.share(|sb| {
        // - add agents
        sb.add_local_agent(&n1, &a1);
        sb.add_local_agent(&n2, &a2);
        sb.add_local_agent(&n3, &a3);
        sb.add_local_agent(&n4, &a4);

        // - add disjoint sets of ops to each node
        sb.add_ops_timed(&n1, true, ops_timed[0..8].to_vec());
        sb.add_ops_timed(&n2, true, ops_timed[8..16].to_vec());
        sb.add_ops_timed(&n3, true, ops_timed[16..24].to_vec());
        sb.add_ops_timed(&n4, true, ops_timed[24..32].to_vec());

        assert_eq!(
            (
                sb.get_ops_loc8(&n1),
                sb.get_ops_loc8(&n2),
                sb.get_ops_loc8(&n3),
                sb.get_ops_loc8(&n4),
            ),
            (
                Loc8::set(ops_only[0..8].to_vec()),
                Loc8::set(ops_only[8..16].to_vec()),
                Loc8::set(ops_only[16..24].to_vec()),
                Loc8::set(ops_only[24..32].to_vec()),
            )
        );

        sb.exchange_all_peer_info();

        sb.print_ascii_arcs(64, true);
    });

    // let gossip do its thing
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    sb.share(|sb| {
        sb.print_ascii_arcs(64, true);

        let mut agent_locs: Vec<_> = vec![a1, a2, a3, a4].into_iter().map(|a| a.loc).collect();
        agent_locs.sort();

        assert_eq!(
            (
                &sb.all_peers(&n1),
                &sb.all_peers(&n2),
                &sb.all_peers(&n3),
                &sb.all_peers(&n4),
            ),
            (&agent_locs, &agent_locs, &agent_locs, &agent_locs)
        );

        // let history = sb
        //     .nodes
        //     .get(&n1)
        //     .unwrap()
        //     .gossip
        //     .state
        //     .share_ref(|s| Ok(s.get_history()))
        //     .unwrap();
        // dbg!(history);

        // dbg!(&sb.nodes, &sb.ops);

        assert_eq!(
            (
                sb.get_ops_loc8(&n1),
                sb.get_ops_loc8(&n2),
                sb.get_ops_loc8(&n3),
                sb.get_ops_loc8(&n4),
            ),
            (
                // NB: in the similar test for recent gossip above, these ranges are
                //     inclusive, but here they are end-exclusive. This is due to
                //     the behavior of arc quantization, where only entire large
                //     segments can be covered, which in this case excludes the endpoint.
                Loc8::set(ops_only[0..16].to_vec()),
                Loc8::set(ops_only[8..24].to_vec()),
                Loc8::set(ops_only[16..32].to_vec()),
                Loc8::set(
                    ops_only[24..32]
                        .to_vec()
                        .into_iter()
                        .chain(ops_only[0..8].to_vec())
                ),
            )
        );
    });
}

/// Set tuning params such that many rounds of gossip happen during the test,
/// to mitigate the false-positive rate inherent to the bloom filters.
fn tuning_params() -> KitsuneP2pTuningParams {
    let mut tp = kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tp.gossip_peer_on_success_next_gossip_delay_ms = 1;
    std::sync::Arc::new(tp)
}
