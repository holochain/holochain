//! A channel-based implementation of network connections, for direct manipulation
//! of the medium of message exchange, used during testing

mod switchboard;
mod switchboard_evt_handler;

#[cfg(test)]
mod tests {
    use kitsune_p2p_types::dht_arc::ArcInterval;

    use super::switchboard::Switchboard;

    #[tokio::test(flavor = "multi_thread")]
    async fn smoke() {
        let mut sb = Switchboard::new();

        let n1 = sb.add_node(Default::default()).await;
        let n2 = sb.add_node(Default::default()).await;
        let n3 = sb.add_node(Default::default()).await;

        sb.space_state()
            .share_mut(|sb, _| {
                sb.add_agent(&n1, 1, ArcInterval::Full);
                sb.add_agent(&n2, 2, ArcInterval::Full);
                sb.add_agent(&n3, 3, ArcInterval::Full);

                sb.add_ops_now(1, true, [2, 3, 4]);
                sb.add_ops_now(2, true, [1, 2]);
                sb.add_ops_now(3, true, [-2, 1]);

                Ok(())
            })
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let all = vec![-2, -1, 1, 2, 3, 4];

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
