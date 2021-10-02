//! A channel-based implementation of network connections, for direct manipulation
//! of the medium of message exchange, used during testing

mod switchboard;
mod switchboard_evt_handler;

#[cfg(test)]
mod tests {
    use kitsune_p2p_types::dht_arc::ArcInterval;

    use super::switchboard::Switchboard;

    #[tokio::test(flavor = "multi_thread")]
    async fn basic_3way_full_sync_switchboard() {
        let mut sb = Switchboard::new();

        let n1 = sb.add_node(Default::default()).await;
        let n2 = sb.add_node(Default::default()).await;
        let n3 = sb.add_node(Default::default()).await;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.add_agent(&n1, 1, ArcInterval::Full);
                sb.add_agent(&n2, 2, ArcInterval::Full);
                sb.add_agent(&n3, 3, ArcInterval::Full);

                sb.add_ops_now(1, true, [10, 20, 30]);
                sb.add_ops_now(2, true, [-10, -20, -30]);
                sb.add_ops_now(3, true, [-15, 15]);

                sb.exchange_peer_info([(&n1, &[2, 3]), (&n2, &[1, 3]), (&n3, &[1, 2])]);

                Ok(())
            })
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let all = vec![-30, -20, -15, -10, 10, 15, 20, 30];

        sb.space_state()
            .share_mut(|sb, _| {
                assert_eq!(sb.get_ops_loc8(&n1), all);
                assert_eq!(sb.get_ops_loc8(&n2), all);
                assert_eq!(sb.get_ops_loc8(&n3), all);
                Ok(())
            })
            .unwrap();
    }
}
