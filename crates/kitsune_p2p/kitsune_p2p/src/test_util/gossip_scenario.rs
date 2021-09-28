//! A channel-based implementation of network connections, for direct manipulation
//! of the medium of message exchange, used during testing

mod gossip_scenario_node;
mod switchboard;

#[cfg(test)]
mod tests {
    use kitsune_p2p_types::dht_arc::ArcInterval;

    use super::switchboard::SwitchboardNetwork;

    #[tokio::test(flavor = "multi_thread")]
    async fn smoke() {
        let mut sb = SwitchboardNetwork::new();

        let n1 = sb.add_endpoint(Default::default()).await;
        let n2 = sb.add_endpoint(Default::default()).await;
        let n3 = sb.add_endpoint(Default::default()).await;

        n1.add_agents([(1, ArcInterval::Full)]);
        n2.add_agents([(2, ArcInterval::Full)]);
        n3.add_agents([(3, ArcInterval::Full)]);
        n1.add_ops([2, 3, 4]);
        n2.add_ops([1, 2]);
        n3.add_ops([-2, -1]);

        let all = vec![-2, -1, 1, 2, 3, 4];
        assert_eq!(n1.get_ops(), all);
        assert_eq!(n2.get_ops(), all);
        assert_eq!(n3.get_ops(), all);

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
}
