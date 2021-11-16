use std::cmp::Ordering;

use super::metrics::Metrics;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A remote node we can connect to.
/// Note that a node can contain many agents.
pub(crate) struct Node {
    pub(crate) agent_info_list: Vec<AgentInfoSigned>,
    pub(crate) cert: Tx2Cert,
    pub(crate) url: TxUrl,
}

impl ShardedGossipLocal {
    /// Find a remote endpoint from agents within arc set.
    pub(super) async fn find_remote_agent_within_arcset(
        &self,
        arc_set: Arc<DhtArcSet>,
        local_agents: &HashSet<Arc<KitsuneAgent>>,
    ) -> KitsuneResult<Option<Node>> {
        let mut remote_nodes: HashMap<Tx2Cert, Node> = HashMap::new();

        // Get all the remote nodes in this arc set.
        let remote_agents_within_arc_set: HashSet<_> =
            store::agents_within_arcset(&self.evt_sender, &self.space, arc_set.clone())
                .await?
                .into_iter()
                .filter(|(a, _)| !local_agents.contains(a))
                .map(|(a, _)| a)
                .collect();

        // Get all the agent info for these remote nodes.
        for info in store::all_agent_info(&self.evt_sender, &self.space)
            .await?
            .into_iter()
            .filter(|a| {
                std::time::Duration::from_millis(a.expires_at_ms)
                    > std::time::UNIX_EPOCH
                        .elapsed()
                        .expect("Your system clock is set before UNIX epoch")
            })
            .filter(|a| remote_agents_within_arc_set.contains(&a.agent))
            .filter(|a| !a.storage_arc.interval().is_empty())
        {
            // Get an address if there is one.
            let info = info
                .url_list
                .iter()
                .filter_map(|url| {
                    kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str())
                        .map_err(|e| tracing::error!("Failed to parse url {:?}", e))
                        .ok()
                        .map(|purl| {
                            (
                                info.clone(),
                                Tx2Cert::from(purl.digest()),
                                TxUrl::from(url.as_str()),
                            )
                        })
                })
                .next();

            // dbg!(&info);

            // If we found a remote address add this agent to the node
            // or create the node if it doesn't exist.
            if let Some((info, cert, url)) = info {
                match remote_nodes.get_mut(&cert) {
                    // Add the agent to the node.
                    Some(node) => node.agent_info_list.push(info),
                    None => {
                        // This is a new node.
                        remote_nodes.insert(
                            cert.clone(),
                            Node {
                                agent_info_list: vec![info],
                                cert,
                                url,
                            },
                        );
                    }
                }
            }
        }

        let remote_nodes = remote_nodes.into_iter().map(|(_, v)| v).collect();
        let tuning_params = self.tuning_params.clone();
        // We could clone the metrics store out of the lock here but I don't think
        // the next_remote_node will be that slow so we can just choose the next node inline.
        self.inner.share_mut(|i, _| {
            let node = next_remote_node(remote_nodes, &i.metrics, tuning_params);
            Ok(node)
        })
    }
}

/// Find the next remote node to sync with.
fn next_remote_node(
    mut remote_nodes: Vec<Node>,
    metrics: &Metrics,
    tuning_params: KitsuneP2pTuningParams,
) -> Option<Node> {
    use rand::prelude::*;
    let mut rng = thread_rng();

    // dbg!(&remote_nodes, metrics);

    // Sort the nodes by longest time since we last successfully gossiped with them.
    // Randomly break ties between nodes we haven't successfully gossiped with.
    // Note the smaller an Instant the longer it is in the past.
    remote_nodes.sort_unstable_by(|a, b| {
        match (
            metrics.last_success(&a.agent_info_list),
            metrics.last_success(&b.agent_info_list),
        ) {
            // Choose the smallest (oldest) Instant.
            (Some(a), Some(b)) => a.cmp(b),
            // Put a behind b that hasn't been gossiped with.
            (Some(_), None) => Ordering::Greater,
            // Put b behind a that hasn't been gossiped with.
            (None, Some(_)) => Ordering::Less,
            // Randomly break ties.
            (None, None) => {
                if rng.gen() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
        }
    });

    remote_nodes
        .into_iter()
        // Don't initiate with nodes we are currently gossiping with.
        .filter(|n| !metrics.is_current_round(&n.agent_info_list))
        .find(|n| {
            match metrics.last_outcome(&n.agent_info_list) {
                Some(metrics::RoundOutcome::Success(when)) => {
                    // If we should force initiate then we don't need to wait for the delay.
                    metrics.forced_initiate()
                        || when.elapsed().as_millis() as u32
                            >= tuning_params.gossip_peer_on_success_next_gossip_delay_ms
                }
                Some(metrics::RoundOutcome::Error(when)) => {
                    when.elapsed().as_millis() as u32
                        >= tuning_params.gossip_peer_on_error_next_gossip_delay_ms
                }
                _ => true,
            }
        })
}

#[cfg(test)]
mod tests {
    use crate::gossip::sharded_gossip::metrics::Metrics;
    use fixt::prelude::*;
    use rand::distributions::Alphanumeric;
    use test_case::test_case;

    use super::*;

    /// Generate a random valid proxy url.
    fn random_url(rng: &mut ThreadRng) -> url2::Url2 {
        let cert_string: String = rng
            .sample_iter(&Alphanumeric)
            .take(39)
            .map(char::from)
            .collect();
        let port = rng.gen_range(5000, 6000);

        url2::url2!(
            "kitsune-proxy://{}mqcw/kitsune-quic/h/localhost/p/{}/-",
            cert_string,
            port
        )
    }

    /// Generate a random pseudo-valid signed agent info
    fn random_agent_info(rng: &mut ThreadRng) -> AgentInfoSigned {
        let space = Arc::new(KitsuneSpace(vec![0x01; 36]));
        let mut agent = vec![0x00; 36];
        rng.fill(&mut agent[..]);
        let agent = Arc::new(KitsuneAgent(agent));

        futures::executor::block_on(AgentInfoSigned::sign(
            space,
            agent,
            42,
            vec![random_url(rng).into()],
            42,
            69,
            |_| async move { Ok(Arc::new(vec![0x03; 64].into())) },
        ))
        .unwrap()
    }

    /// Tuning params with no delay on recently gossiped to nodes.
    fn tuning_params_no_delay() -> KitsuneP2pTuningParams {
        let mut t = tuning_params_struct::KitsuneP2pTuningParams::default();
        t.gossip_peer_on_success_next_gossip_delay_ms = 0;
        t.gossip_peer_on_error_next_gossip_delay_ms = 0;
        Arc::new(t)
    }

    /// Tuning params with a delay on recently gossiped to nodes.
    fn tuning_params_delay(success: u32, error: u32) -> KitsuneP2pTuningParams {
        let mut t = tuning_params_struct::KitsuneP2pTuningParams::default();
        t.gossip_peer_on_success_next_gossip_delay_ms = success;
        t.gossip_peer_on_error_next_gossip_delay_ms = error;
        Arc::new(t)
    }

    fn create_remote_nodes(n: usize) -> Vec<Node> {
        let mut rng = thread_rng();
        (0..n)
            .map(|_| {
                let info = random_agent_info(&mut rng);
                let url = info.url_list.get(0).unwrap().clone();
                let url = TxUrl::from(url.as_str());
                let purl = kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str()).unwrap();
                Node {
                    agent_info_list: vec![info],
                    cert: Tx2Cert::from(purl.digest()),
                    url,
                }
            })
            .collect()
    }

    #[test]
    /// Test that we can find a remote node to sync with
    /// when there is only one to choose from.
    fn next_remote_node_sanity() {
        // - Create one remote node.
        let remote_nodes = create_remote_nodes(1);

        let r = next_remote_node(
            remote_nodes.clone(),
            &Default::default(),
            tuning_params_no_delay(),
        );

        // - That node is chosen.
        assert_eq!(r, remote_nodes.first().cloned());
    }

    /// Test that given N remote nodes we choose the one
    /// we talked to the least recently.
    #[test_case(1)]
    #[test_case(2)]
    #[test_case(10)]
    #[test_case(100)]
    fn next_remote_node_least_recently(n: usize) {
        // - Create N remote nodes.
        let mut remote_nodes = create_remote_nodes(n);

        let mut metrics = Metrics::new();

        // - Pop the last node off the list.
        let last = remote_nodes.pop().unwrap();

        // - Record a successful initiate round for the last node at the earliest time.
        metrics.record_initiate(&last.agent_info_list);
        metrics.record_success(&last.agent_info_list);

        // - Record successful initiate rounds for the rest of the nodes at later times.
        for node in remote_nodes.iter() {
            metrics.record_initiate(&node.agent_info_list);
            metrics.record_success(&node.agent_info_list);
        }

        // - Push the last node back into the remote nodes.
        remote_nodes.push(last);

        let r = next_remote_node(remote_nodes.clone(), &metrics, tuning_params_no_delay());

        // - Expect the last node to be chosen because it was the least recently gossiped with.
        assert_eq!(r, remote_nodes.last().cloned());
    }

    /// Test that given N remote nodes we choose the one
    /// we've never talked to before over all others.
    #[test_case(1)]
    #[test_case(2)]
    #[test_case(10)]
    #[test_case(100)]
    fn next_remote_node_never_talked_to(n: usize) {
        // - Create N remote nodes.
        let mut remote_nodes = create_remote_nodes(n);

        let mut metrics = Metrics::new();

        // - Pop the last node off the list.
        let last = remote_nodes.pop().unwrap();

        // - Record successful initiate rounds for the rest of the nodes.
        for node in remote_nodes.iter() {
            metrics.record_initiate(&node.agent_info_list);
            metrics.record_success(&node.agent_info_list);
        }

        // - Push the last node back into the remote nodes.
        remote_nodes.push(last);

        let r = next_remote_node(remote_nodes.clone(), &metrics, tuning_params_no_delay());

        // - Expect the last node to be chosen because it was never gossiped with.
        assert_eq!(r, remote_nodes.last().cloned());
    }

    #[test]
    /// Test we break ties between never talked
    /// to nodes by randomly choosing one.
    fn randomly_break_ties() {
        // - Create 100 remote nodes.
        let mut remote_nodes = create_remote_nodes(100);

        let mut metrics = Metrics::new();

        // - Pop the last two nodes off the list.
        let last = remote_nodes.pop().unwrap();
        let second_last = remote_nodes.pop().unwrap();

        // - Record successful initiate rounds for the rest of the nodes.
        for node in remote_nodes.iter() {
            metrics.record_initiate(&node.agent_info_list);
            metrics.record_success(&node.agent_info_list);
        }

        // - Push the last two nodes back into the remote nodes.
        remote_nodes.push(second_last.clone());
        remote_nodes.push(last.clone());

        // - Check we don't always get the same node.
        let mut chose_last = false;
        let mut chose_second_last = false;
        for _ in 0..100 {
            let r =
                next_remote_node(remote_nodes.clone(), &metrics, tuning_params_no_delay()).unwrap();
            if r == last {
                chose_last = true;
            } else if r == second_last {
                chose_second_last = true;
            }
        }
        assert!(chose_last && chose_second_last);
    }

    /// Test that given N remote nodes we never choose a current round.
    #[test_case(1)]
    #[test_case(2)]
    #[test_case(10)]
    #[test_case(100)]
    fn dont_choose_current_rounds(n: usize) {
        // - Create N remote nodes.
        let mut remote_nodes = create_remote_nodes(n);

        let mut metrics = Metrics::new();

        // - Pop the last node off the list.
        let last = remote_nodes.pop().unwrap();

        // - Record remote rounds for the rest of the nodes
        // but don't record any successes.
        for node in remote_nodes.iter() {
            metrics.record_remote_round(&node.agent_info_list);
        }

        let r = next_remote_node(remote_nodes.clone(), &metrics, tuning_params_no_delay());

        // - Without the last node we expect no nodes to be chosen.
        assert!(r.is_none());

        // - Record the last node as a successful round and push it into the list.
        metrics.record_initiate(&last.agent_info_list);
        metrics.record_success(&last.agent_info_list);
        remote_nodes.push(last);

        let r = next_remote_node(remote_nodes.clone(), &metrics, tuning_params_no_delay());

        // - Now we expect the last node to be chosen.
        // (because we're using "no delay" for the tuning params)
        assert_eq!(r, remote_nodes.last().cloned());
    }

    #[test]
    /// Test we don't choose nodes we've seen too recently.
    fn dont_choose_very_recent_rounds() {
        // - Create 100 remote nodes.
        let remote_nodes = create_remote_nodes(100);

        let mut metrics = Metrics::new();

        // - Record successful initiate rounds for the all of the nodes.
        for node in remote_nodes.iter() {
            metrics.record_initiate(&node.agent_info_list);
            metrics.record_success(&node.agent_info_list);
        }

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Expect no nodes to be chosen.
        assert!(r.is_none());

        // - Use up 10 ms.
        std::thread::sleep(Duration::from_millis(10));

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Still no result.
        assert!(r.is_none());

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a 9 ms after the successful round.
            tuning_params_delay(9, 0),
        );

        // - Now we should get a result.
        assert!(r.is_some());

        // - Record error outcomes for every node.
        for node in remote_nodes.iter() {
            metrics.record_initiate(&node.agent_info_list);
            metrics.record_error(&node.agent_info_list);
        }

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(0, 1000 * 60),
        );

        // - Expect no nodes to be chosen.
        assert!(r.is_none());

        // - Use up 10ms.
        std::thread::sleep(Duration::from_millis(10));

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(0, 1000 * 60),
        );

        // - Still no result.
        assert!(r.is_none());

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a 9 ms after an error round.
            tuning_params_delay(1000 * 60, 9),
        );

        // - Now we should get a result.
        assert!(r.is_some());
    }

    /// Test that given N remote nodes and a force initiate trigger
    /// we will choose the least recent node even if it's too recent.
    #[test_case(1)]
    #[test_case(2)]
    #[test_case(10)]
    #[test_case(100)]
    fn force_initiate(n: usize) {
        // - Create N remote nodes.
        let mut remote_nodes = create_remote_nodes(n);

        let mut metrics = Metrics::new();

        // - Pop the last node off the list.
        let last = remote_nodes.pop().unwrap();

        // - Record a successful initiate round for the last node before the other nodes.
        metrics.record_initiate(&last.agent_info_list);
        metrics.record_success(&last.agent_info_list);

        // - Record successful initiate rounds for the rest of the nodes.
        for node in remote_nodes.iter() {
            metrics.record_initiate(&node.agent_info_list);
            metrics.record_success(&node.agent_info_list);
        }

        // - Push the last node back on the list.
        remote_nodes.push(last);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Expect no nodes to be chosen.
        assert!(r.is_none());

        // - First force initiate.
        metrics.record_force_initiate();

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Expect the last node to be chosen because it was successfully gossiped
        // with the lest recently and we are force initiating.
        assert_eq!(r, remote_nodes.last().cloned());

        // - Record this successful initiate round.
        let last = remote_nodes.last().unwrap();
        metrics.record_initiate(&last.agent_info_list);
        metrics.record_success(&last.agent_info_list);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Now the first node is the least recently gossiped with.
        assert_eq!(r, remote_nodes.first().cloned());

        // - Record this successful initiate round.
        let first = remote_nodes.first().unwrap();
        metrics.record_initiate(&first.agent_info_list);
        metrics.record_success(&first.agent_info_list);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );
        // - Force initiate only forces 2 nodes so now we expect no nodes
        // to be chosen because they are all more recent then the tuning params delay.
        assert!(r.is_none());

        // - Second force initiate.
        metrics.record_force_initiate();

        // Helper function to get the next expected node.
        let expected_node = |i| {
            match n {
                // - Only one node so the first will always be chosen.
                1 => remote_nodes.first(),
                // Two nodes so it will alternate between the first and last.
                2 => {
                    if i % 2 == 0 {
                        remote_nodes.first()
                    } else {
                        remote_nodes.last()
                    }
                }
                // All other tests will climb in the order they recorded success.
                _ => remote_nodes.get(i),
            }
        };

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Now we expect node 1 to be chosen (unless there is only one node).
        assert_eq!(r, expected_node(1).cloned());

        // - Record the successful initiate round for this node.
        let node = expected_node(1).unwrap();
        metrics.record_initiate(&node.agent_info_list);
        metrics.record_success(&node.agent_info_list);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Now we expect node 2 to be chosen (unless there is only one node).
        assert_eq!(r, expected_node(2).cloned());

        // - Record the successful initiate round for this node.
        let node = expected_node(2).unwrap();
        metrics.record_initiate(&node.agent_info_list);
        metrics.record_success(&node.agent_info_list);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - We expect no nodes to be chosen because the forced initiate has run out.
        assert!(r.is_none());

        // - Third force initiate.
        metrics.record_force_initiate();

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Now we expect node 3 to be chosen (unless there is only one or two nodes).
        assert_eq!(r, expected_node(3).cloned());

        // - Record the successful initiate round for this node.
        let node = expected_node(3).unwrap();
        metrics.record_initiate(&node.agent_info_list);
        metrics.record_success(&node.agent_info_list);

        // - Forth force initiate overlaps with third so it resets.
        metrics.record_force_initiate();

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Now we expect node 4 to be chosen (unless there is only one or two nodes).
        assert_eq!(r, expected_node(4).cloned());

        // - Record the successful initiate round for this node.
        let node = expected_node(4).unwrap();
        metrics.record_initiate(&node.agent_info_list);
        metrics.record_success(&node.agent_info_list);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - We expect the 5 node to be chosen because the forced initiate was reset.
        assert_eq!(r, expected_node(5).cloned());

        // - Record the successful initiate round for this node.
        let node = expected_node(5).unwrap();
        metrics.record_initiate(&node.agent_info_list);
        metrics.record_success(&node.agent_info_list);

        let r = next_remote_node(
            remote_nodes.clone(),
            &metrics,
            // - Set the tuning params to a delay in the future.
            tuning_params_delay(1000 * 60, 0),
        );

        // - Now the reset has run out we get no nodes.
        assert!(r.is_none());
    }
}
