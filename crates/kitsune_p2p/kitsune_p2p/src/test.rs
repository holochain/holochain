#[cfg(test)]
mod tests {
    use crate::{event::*, types::actor::KitsuneP2pSender, *};
    use futures::future::FutureExt;
    use ghost_actor::{dependencies::tracing, GhostControlSender};
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn test_transport_binding() -> Result<(), KitsuneP2pError> {
        let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .finish(),
        );

        // Create a p2p config with a local proxy that rejects proxying anyone else
        // and binds to `kitsune-quic://0.0.0.0:0`.
        // This allows the OS to assign a port.
        let mut config = KitsuneP2pConfig::default();
        config.transport_pool.push(TransportConfig::Proxy {
            sub_transport: Box::new(TransportConfig::Quic {
                bind_to: Some(url2::url2!("kitsune-quic://0.0.0.0:0")),
                override_host: None,
                override_port: None,
            }),
            proxy_config: ProxyConfig::LocalProxyServer {
                proxy_accept_config: Some(ProxyAcceptConfig::RejectAll),
            },
        });
        // Spawn the kitsune p2p actor that will respond to listing bindings.
        let (p2p, _evt) = spawn_kitsune_p2p(config).await.unwrap();
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
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_request_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());
        let a2: Arc<KitsuneAgent> =
            Arc::new(b"222222222222222222222222222222222222".to_vec().into());

        let (p2p, mut evt) = spawn_kitsune_p2p(KitsuneP2pConfig::default())
            .await
            .unwrap();

        let space1_clone = space1.clone();
        let a2_clone = a2.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use KitsuneP2pEvent::*;
                match evt {
                    Call {
                        respond,
                        space,
                        to_agent,
                        payload,
                        ..
                    } => {
                        if space != space1_clone {
                            panic!("unexpected space");
                        }
                        if to_agent != a2_clone {
                            panic!("unexpected agent");
                        }
                        if &*payload != b"hello" {
                            panic!("unexpected request");
                        }
                        respond.r(Ok(async move { Ok(b"echo: hello".to_vec()) }
                            .boxed()
                            .into()));
                    }
                    _ => (),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();

        let res = p2p
            .rpc_single(space1, a2, a1, b"hello".to_vec())
            .await
            .unwrap();
        assert_eq!(b"echo: hello".to_vec(), res);

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_broadcast_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());
        let a2: Arc<KitsuneAgent> =
            Arc::new(b"222222222222222222222222222222222222".to_vec().into());
        let a3: Arc<KitsuneAgent> =
            Arc::new(b"333333333333333333333333333333333333".to_vec().into());

        let (p2p, mut evt) = spawn_kitsune_p2p(KitsuneP2pConfig::default())
            .await
            .unwrap();

        let recv_count = Arc::new(std::sync::atomic::AtomicU8::new(0));

        let space1_clone = space1.clone();
        let recv_count_clone = recv_count.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use KitsuneP2pEvent::*;
                match evt {
                    Notify {
                        respond,
                        space,
                        payload,
                        ..
                    } => {
                        if space != space1_clone {
                            panic!("unexpected space");
                        }
                        if &*payload != b"test-broadcast" {
                            panic!("unexpected request");
                        }
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                        recv_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    _ => (),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();
        p2p.join(space1.clone(), a3.clone()).await.unwrap();

        let res = p2p
            .notify_multi(actor::NotifyMulti {
                space: space1,
                from_agent: a1,
                // this is just a dummy value right now
                basis: Arc::new(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_vec().into()),
                remote_agent_count: Some(42),
                timeout_ms: Some(40),
                payload: b"test-broadcast".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(3, res);
        assert_eq!(3, recv_count.load(std::sync::atomic::Ordering::SeqCst));

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_multi_request_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());
        let a2: Arc<KitsuneAgent> =
            Arc::new(b"222222222222222222222222222222222222".to_vec().into());
        let a3: Arc<KitsuneAgent> =
            Arc::new(b"333333333333333333333333333333333333".to_vec().into());

        let (p2p, mut evt) = spawn_kitsune_p2p(KitsuneP2pConfig::default())
            .await
            .unwrap();

        let space1_clone = space1.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use KitsuneP2pEvent::*;
                match evt {
                    Call {
                        respond,
                        space,
                        payload,
                        ..
                    } => {
                        if space != space1_clone {
                            panic!("unexpected space");
                        }
                        let payload = String::from_utf8_lossy(&payload);
                        assert_eq!(&payload, "test-multi-request");
                        respond.r(Ok(async move { Ok(b"echo: test-multi-request".to_vec()) }
                            .boxed()
                            .into()));
                    }
                    _ => (),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();
        p2p.join(space1.clone(), a3.clone()).await.unwrap();

        let res = p2p
            .rpc_multi(actor::RpcMulti {
                space: space1,
                from_agent: a1.clone(),
                // this is just a dummy value right now
                basis: Arc::new(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_vec().into()),
                remote_agent_count: Some(2),
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
            assert!(r.agent == a2 || r.agent == a3);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_single_agent_multi_request_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());

        let (p2p, mut evt) = spawn_kitsune_p2p(KitsuneP2pConfig::default())
            .await
            .unwrap();

        let space1_clone = space1.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use KitsuneP2pEvent::*;
                match evt {
                    Call {
                        respond,
                        space,
                        payload,
                        ..
                    } => {
                        if space != space1_clone {
                            panic!("unexpected space");
                        }
                        let payload = String::from_utf8_lossy(&payload);
                        assert_eq!(&payload, "test-multi-request");
                        respond.r(Ok(async move { Ok(b"echo: test-multi-request".to_vec()) }
                            .boxed()
                            .into()));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();

        let res = p2p
            .rpc_multi(actor::RpcMulti {
                space: space1,
                from_agent: a1.clone(),
                // this is just a dummy value right now
                basis: Arc::new(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_vec().into()),
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

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_gossip_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());
        let a2: Arc<KitsuneAgent> =
            Arc::new(b"222222222222222222222222222222222222".to_vec().into());

        let oh1: Arc<KitsuneOpHash> =
            Arc::new(b"oooooooooooooooooooooooooooooooooooo".to_vec().into());
        let oh2: Arc<KitsuneOpHash> =
            Arc::new(b"hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh".to_vec().into());

        let (p2p, mut evt) = spawn_kitsune_p2p(KitsuneP2pConfig::default())
            .await
            .unwrap();

        let result = Arc::new(std::sync::RwLock::new((false, false)));

        //let space1_clone = space1.clone();
        let a1_clone = a1.clone();
        //let a2_clone = a2.clone();
        let result_clone = result.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use KitsuneP2pEvent::*;
                match evt {
                    FetchOpHashesForConstraints { respond, input, .. } => {
                        //println!("FETCH HASHES REQ: {:#?}", input);
                        let oh = if input.agent == a1_clone {
                            oh1.clone()
                        } else {
                            oh2.clone()
                        };
                        respond.r(Ok(async move { Ok(vec![oh]) }.boxed().into()));
                    }
                    FetchOpHashData { respond, input, .. } => {
                        //println!("FETCH HASH DATA REQ: {:#?}", input);
                        let mut out = Vec::new();
                        for op_hash in input.op_hashes {
                            out.push((op_hash, vec![]));
                        }
                        respond.r(Ok(async move { Ok(out) }.boxed().into()));
                    }
                    Gossip {
                        respond,
                        //agent,
                        op_hash,
                        ..
                    } => {
                        //println!("GOT GOSSIP: {:?} {:?}", agent, op_hash);
                        if op_hash == oh1 {
                            result_clone.write().unwrap().0 = true;
                        } else {
                            result_clone.write().unwrap().1 = true;
                        }
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    _ => (),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();

        let is_ok = move || {
            let lock = result.read().unwrap();
            lock.0 && lock.1
        };

        for _ in 0..10 {
            if is_ok() {
                break;
            }
            tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();

        if !is_ok() {
            panic!("failed to gossip both dht op hashes");
        }
    }
}
