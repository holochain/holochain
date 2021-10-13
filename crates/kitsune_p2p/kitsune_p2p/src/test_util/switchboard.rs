//! A channel-based implementation of network connections, for direct manipulation
//! of the medium of message exchange, used during testing

mod switchboard;
mod switchboard_evt_handler;

#[cfg(test)]
mod tests {
    use kitsune_p2p_timestamp::Timestamp;
    use kitsune_p2p_types::dht_arc::{loc8::Loc8, ArcInterval};

    use crate::gossip::sharded_gossip::GossipType;

    use super::switchboard::Switchboard;
    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread")]
    async fn basic_3way_full_sync_switchboard() {
        // observability::test_run().ok();
        let mut sb = Switchboard::new(GossipType::Recent);

        let [n1, n2, n3] = sb.add_nodes().await;

        // These become agent locations, but currently kitsune doesn't care
        // where the agents are located, it only cares about their storage arc
        let a1 = 1;
        let a2 = 2;
        let a3 = 3;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.add_local_agent(&n1, a1, ArcInterval::Full);
                sb.add_local_agent(&n2, a2, ArcInterval::Full);
                sb.add_local_agent(&n3, a3, ArcInterval::Full);

                sb.add_ops_now(a1, true, [10, 20, 30]);
                sb.add_ops_now(a2, true, [-10, -20, -30]);
                sb.add_ops_now(a3, true, [-15, 15]);

                // we wouldn't expect this op to be gossiped, since it's from 50+ years ago
                // and hardly "recent"
                sb.add_ops_timed(3, true, [(40, Timestamp::from_micros(1))]);

                sb.exchange_all_peer_info();

                // Ensure that the initial conditions are set up properly
                assert_eq!(sb.get_ops_loc8(&n1), Loc8::vec([10, 20, 30]));
                assert_eq!(sb.get_ops_loc8(&n2), Loc8::vec([-30, -20, -10]));
                assert_eq!(sb.get_ops_loc8(&n3), Loc8::vec([-15, 15, 40]));

                Ok(())
            })
            .unwrap();

        // let gossip do its thing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let most = Loc8::vec([-30, -20, -15, -10, 10, 15, 20, 30]);
        let mut all = most.clone();
        all.extend(Loc8::vec([40]));

        sb.space_state()
            .share_mut(|sb, _| {
                assert_eq!(sb.get_ops_loc8(&n1), most);
                assert_eq!(sb.get_ops_loc8(&n2), most);
                assert_eq!(sb.get_ops_loc8(&n3), all);
                Ok(())
            })
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn basic_3way_sharded_switchboard() {
        observability::test_run().ok();
        let mut sb = Switchboard::new(GossipType::Recent);

        let [n1, n2, n3] = sb.add_nodes().await;

        // These become agent locations, but currently kitsune doesn't care
        // where the agents are located, it only cares about their storage arc
        let a1 = 1;
        let a2 = 2;
        let a3 = 3;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.add_local_agent(&n1, a1, ArcInterval::Bounded(-30, 90));
                sb.add_local_agent(&n2, a2, ArcInterval::Bounded(-90, 30));
                sb.add_local_agent(&n3, a3, ArcInterval::Bounded(60, -60));

                sb.add_ops_now(a1, true, [10, 20, 30, 40, 50, 60, 70, 80]);
                sb.add_ops_now(a2, true, [-10, -20, -30, -40, -50, -60, -70, -80]);
                sb.add_ops_now(a3, true, [90, 120, -120, -90]);

                sb.exchange_all_peer_info();

                Ok(())
            })
            .unwrap();

        // let gossip do its thing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.print_ascii_arcs(32);
                assert_eq!(
                    (
                        sb.get_ops_loc8(&n1),
                        sb.get_ops_loc8(&n2),
                        sb.get_ops_loc8(&n3)
                    ),
                    (
                        Loc8::vec([-30, -20, -10, 10, 20, 30, 40, 50, 60, 70, 80, 90]),
                        Loc8::vec([-90, -80, -70, -60, -50, -40, -30, -20, -10, 10, 20, 30]),
                        Loc8::vec([-120, -90, -80, -70, -60, 60, 70, 80, 90, 120]),
                    )
                );
                Ok(())
            })
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn peer_gossip() {
        observability::test_run().ok();
        let mut sb = Switchboard::new(GossipType::Recent);

        let [n1, n2, n3] = sb.add_nodes().await;

        // These become agent locations, but currently kitsune doesn't care
        // where the agents are located, it only cares about their storage arc
        let a1 = 30;
        let a2 = -30;
        let a3 = 120;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.add_local_agent(&n1, a1, ArcInterval::Bounded(-30, 90));
                sb.add_local_agent(&n2, a2, ArcInterval::Bounded(-90, 30));
                sb.add_local_agent(&n3, a3, ArcInterval::Bounded(60, -60));

                sb.add_ops_now(a1, true, [10, 20, 30, 40, 50, 60, 70, 80]);
                sb.add_ops_now(a2, true, [-10, -20, -30, -40, -50, -60, -70, -80]);
                sb.add_ops_now(a3, true, [90, 120, -120, -90]);

                sb.inject_peer_info(&n1, [a2]);
                sb.inject_peer_info(&n2, [a3]);
                sb.inject_peer_info(&n3, [a1]);

                Ok(())
            })
            .unwrap();

        // let gossip do its thing
        tokio::time::sleep(tokio::time::Duration::from_millis(10000)).await;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.print_ascii_arcs(32);
                assert_eq!(
                    (
                        sb.get_ops_loc8(&n1),
                        sb.get_ops_loc8(&n2),
                        sb.get_ops_loc8(&n3)
                    ),
                    (
                        Loc8::vec([-30, -20, -10, 10, 20, 30, 40, 50, 60, 70, 80, 90]),
                        Loc8::vec([-90, -80, -70, -60, -50, -40, -30, -20, -10, 10, 20, 30]),
                        Loc8::vec([-120, -90, -80, -70, -60, 60, 70, 80, 90, 120]),
                    )
                );
                Ok(())
            })
            .unwrap();
    }
}
