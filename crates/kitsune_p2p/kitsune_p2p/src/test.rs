#[cfg(test)]
mod tests {
    use crate::test_util::*;
    use crate::types::actor::KitsuneP2pSender;
    use crate::*;
    use ghost_actor::dependencies::tracing;
    use ghost_actor::GhostControlSender;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_transport_coms() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();
        let (harness, _evt) = spawn_test_harness_mem().await?;

        let space = harness.add_space().await?;
        let (a1, p2p1) = harness.add_direct_agent("one".into()).await?;
        let (a2, p2p2) = harness.add_direct_agent("two".into()).await?;

        // needed until we have some way of bootstrapping
        harness.magic_peer_info_exchange().await?;

        let r1 = p2p1
            .rpc_single(space.clone(), a2.clone(), a1.clone(), b"m1".to_vec(), None)
            .await?;
        let r2 = p2p2
            .rpc_single(space.clone(), a1, a2, b"m2".to_vec(), None)
            .await?;
        assert_eq!(b"echo: m1".to_vec(), r1);
        assert_eq!(b"echo: m2".to_vec(), r2);
        harness.ghost_actor_shutdown().await?;
        crate::types::metrics::print_all_metrics();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_transport_multi_coms() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();
        let (harness, _evt) = spawn_test_harness_mem().await?;

        let space = harness.add_space().await?;
        let (a1, p2p1) = harness.add_direct_agent("one".into()).await?;
        let (a2, _p2p2) = harness.add_direct_agent("two".into()).await?;
        let (a3, _p2p3) = harness.add_direct_agent("tre".into()).await?;

        // needed until we have some way of bootstrapping
        harness.magic_peer_info_exchange().await?;

        let res = p2p1
            .rpc_multi(actor::RpcMulti {
                space: space,
                from_agent: a1.clone(),
                // this is just a dummy value right now
                basis: TestVal::test_val(),
                remote_agent_count: Some(5),
                timeout_ms: Some(200),
                as_race: true,
                race_timeout_ms: Some(100),
                payload: b"test-multi-request".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(3, res.len());
        for r in res {
            let data = String::from_utf8_lossy(&r.response);
            assert_eq!("echo: test-multi-request", &data);
            assert!(r.agent == a1 || r.agent == a2 || r.agent == a3);
        }

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_transport_notify_coms() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();
        let (harness, evt) = spawn_test_harness_mem().await?;
        let mut rcv = evt.receive();

        let space = harness.add_space().await?;
        let (a1, p2p1) = harness.add_direct_agent("one".into()).await?;
        let (_a2, _p2p2) = harness.add_direct_agent("two".into()).await?;
        let (_a3, _p2p3) = harness.add_direct_agent("tre".into()).await?;

        // needed until we have some way of bootstrapping
        harness.magic_peer_info_exchange().await?;

        p2p1.notify_multi(actor::NotifyMulti {
            space: space,
            from_agent: a1,
            // this is just a dummy value right now
            basis: TestVal::test_val(),
            remote_agent_count: Some(42),
            timeout_ms: Some(40),
            payload: b"test-broadcast".to_vec(),
        })
        .await?;

        harness.ghost_actor_shutdown().await?;

        let mut recv_count = 0_usize;
        while let Some(evt) = tokio_stream::StreamExt::next(&mut rcv).await {
            if let test_util::HarnessEventType::Notify { payload, .. } = &evt.ty {
                assert_eq!(&**payload, "test-broadcast");
                recv_count += 1;
            }
        }

        assert_eq!(3, recv_count);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_peer_info_store() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, evt) = spawn_test_harness_mem().await?;
        let mut recv = evt.receive();

        harness.add_space().await?;
        let (agent, _p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        harness.ghost_actor_shutdown().await?;

        let mut agent_info_signed = None;

        use tokio_stream::StreamExt;
        while let Some(item) = recv.next().await {
            if let HarnessEventType::StoreAgentInfo { agent, .. } = item.ty {
                agent_info_signed = Some((agent,));
            }
        }

        if let Some(i) = agent_info_signed {
            assert_eq!(i.0, Slug::from(agent));
            return Ok(());
        }

        panic!("Failed to receive agent_info_signed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_transport_binding() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        // Create a p2p config with a local proxy that rejects proxying anyone else
        // and binds to `kitsune-quic://0.0.0.0:0`.
        // This allows the OS to assign an interface / port.
        harness.add_space().await?;
        let (_, p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        // List the bindings and assert that we have one binding that is a
        // kitsune-proxy scheme with a kitsune-quic url.
        let bindings = p2p.list_transport_bindings().await?;
        tracing::warn!("BINDINGS: {:?}", bindings);
        assert_eq!(1, bindings.len());
        let binding = &bindings[0];
        assert_eq!("kitsune-proxy", binding.scheme());
        assert_eq!(
            "kitsune-quic",
            binding.path_segments().unwrap().next().unwrap()
        );

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;
        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone()).await?;

        let res = p2p
            .rpc_single(space, a2, a1, b"hello".to_vec(), None)
            .await?;
        assert_eq!(b"echo: hello".to_vec(), res);

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_broadcast_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run_open().ok();
        let span = tracing::debug_span!("test");
        let _g = span.enter();
        observability::span_context!(span);

        let (harness, evt) = spawn_test_harness_quic().await?;
        let mut rcv = evt.receive();

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone()).await?;
        let a3: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a3.clone()).await?;

        p2p.notify_multi(actor::NotifyMulti {
            space: space,
            from_agent: a1,
            // this is just a dummy value right now
            basis: TestVal::test_val(),
            remote_agent_count: Some(42),
            timeout_ms: Some(40),
            payload: b"test-broadcast".to_vec(),
        })
        .await?;

        harness.ghost_actor_shutdown().await?;

        let mut recv_count = 0_usize;
        while let Some(evt) = tokio_stream::StreamExt::next(&mut rcv).await {
            if &**evt.nick != "DIRECT" {
                continue;
            }
            if let test_util::HarnessEventType::Notify { payload, .. } = &evt.ty {
                assert_eq!(&**payload, "test-broadcast");
                recv_count += 1;
            }
        }

        assert_eq!(3, recv_count);

        observability::span_context!(span);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_multi_request_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone()).await?;
        let a3: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a3.clone()).await?;

        let res = p2p
            .rpc_multi(actor::RpcMulti {
                space: space,
                from_agent: a1.clone(),
                // this is just a dummy value right now
                basis: TestVal::test_val(),
                remote_agent_count: Some(2),
                timeout_ms: Some(20),
                as_race: true,
                race_timeout_ms: Some(20),
                payload: b"test-multi-request".to_vec(),
            })
            .await
            .unwrap();

        harness.ghost_actor_shutdown().await?;

        assert_eq!(3, res.len());
        for r in res {
            let data = String::from_utf8_lossy(&r.response);
            assert_eq!("echo: test-multi-request", &data);
            assert!(r.agent == a1 || r.agent == a2 || r.agent == a3);
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_single_agent_multi_request_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        let res = p2p
            .rpc_multi(actor::RpcMulti {
                space: space,
                from_agent: a1.clone(),
                // this is just a dummy value right now
                basis: TestVal::test_val(),
                remote_agent_count: Some(1),
                timeout_ms: Some(20),
                as_race: true,
                race_timeout_ms: Some(20),
                payload: b"test-multi-request".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(1, res.len());
        for r in res {
            let data = String::from_utf8_lossy(&r.response);
            assert_eq!("echo: test-multi-request", &data);
            assert!(r.agent == a1);
        }

        harness.ghost_actor_shutdown().await.unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_gossip_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone()).await?;

        let op1 = harness
            .inject_gossip_data(a1.clone(), "agent-1-data".to_string())
            .await?;

        // TODO - This doesn't work on fake nodes
        //        we need to actually add_*_agent to do this
        //let op2 = harness.inject_gossip_data(a2, "agent-2-data".to_string()).await?;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let res = harness.dump_local_gossip_data(a1).await?;
        let (op_hash, data) = res.into_iter().next().unwrap();
        assert_eq!(op1, op_hash);
        assert_eq!("agent-1-data", &data);

        // TODO - This doesn't work on fake nodes
        //        we need to actually add_*_agent to do this
        //let res = harness.dump_local_gossip_data(a2).await?;
        //let (op_hash, data) = res.into_iter().next().unwrap();
        //assert_eq!(op2, op_hash);
        //assert_eq!("agent-2-data", &data);

        harness.ghost_actor_shutdown().await.unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_peer_data_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        let res = harness.dump_local_peer_data(a1.clone()).await?;
        let num_agent_info = res.len();
        let (agent_hash, _agent_info) = res.into_iter().next().unwrap();
        assert_eq!(a1, agent_hash);
        assert_eq!(num_agent_info, 1);

        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone()).await?;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let res = harness.dump_local_peer_data(a1.clone()).await?;
        let num_agent_info = res.len();

        assert!(res.contains_key(&a1));
        assert!(res.contains_key(&a2));
        assert_eq!(num_agent_info, 2);

        harness.ghost_actor_shutdown().await.unwrap();
        Ok(())
    }

    /// Test that we can gossip across a in memory transport layer.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_gossip_transport() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();
        let (harness, _evt) = spawn_test_harness_mem().await?;

        harness.add_space().await?;

        // - Add the first agent
        let (a1, _) = harness.add_direct_agent("one".into()).await?;

        // - Insert some data for agent 1
        let op1 = harness
            .inject_gossip_data(a1.clone(), "agent-1-data".to_string())
            .await?;

        // - Check agent one has the data
        let res = harness.dump_local_gossip_data(a1.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op1);
        assert_eq!(Some(&"agent-1-data".to_string()), data);
        assert_eq!(num_gossip, 1);

        // - Add the second agent
        let (a2, _) = harness.add_direct_agent("two".into()).await?;

        // - Insert some data for agent 2
        let op2 = harness
            .inject_gossip_data(a2.clone(), "agent-2-data".to_string())
            .await?;

        // - Check agent two only has this data
        let res = harness.dump_local_gossip_data(a2.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op2);
        assert_eq!(Some(&"agent-2-data".to_string()), data);
        assert_eq!(num_gossip, 1);

        // TODO: remove when we have bootstrapping for tests
        // needed until we have some way of bootstrapping
        harness.magic_peer_info_exchange().await?;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // - Check agent one now has all the data
        let res = harness.dump_local_gossip_data(a1.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op1);
        assert_eq!(Some(&"agent-1-data".to_string()), data);
        let data = res.get(&op2);
        assert_eq!(Some(&"agent-2-data".to_string()), data);
        assert_eq!(num_gossip, 2);

        // - Check agent two now has all the data
        let res = harness.dump_local_gossip_data(a2.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op1);
        assert_eq!(Some(&"agent-1-data".to_string()), data);
        let data = res.get(&op2);
        assert_eq!(Some(&"agent-2-data".to_string()), data);
        assert_eq!(num_gossip, 2);

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }
}
