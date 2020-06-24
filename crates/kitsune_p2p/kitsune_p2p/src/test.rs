#[cfg(test)]
mod tests {
    use crate::{event::*, spawn::*, types::*};
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn test_request_workflow() {
        let space1: Arc<KitsuneSpace> =
            Arc::new(b"ssssssssssssssssssssssssssssssssssss".to_vec().into());
        let a1: Arc<KitsuneAgent> =
            Arc::new(b"111111111111111111111111111111111111".to_vec().into());
        let a2: Arc<KitsuneAgent> =
            Arc::new(b"222222222222222222222222222222222222".to_vec().into());

        let (mut p2p, mut evt) = spawn_kitsune_p2p().await.unwrap();

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
                        agent,
                        payload,
                        ..
                    } => {
                        if space != space1_clone {
                            panic!("unexpected space");
                        }
                        if agent != a2_clone {
                            panic!("unexpected agent");
                        }
                        if &*payload != b"hello" {
                            panic!("unexpected request");
                        }
                        let _ = respond(Ok(b"echo: hello".to_vec()));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();

        let res = p2p.rpc_single(space1, a2, b"hello".to_vec()).await.unwrap();
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

        let (mut p2p, mut evt) = spawn_kitsune_p2p().await.unwrap();

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
                        let _ = respond(Ok(()));
                        recv_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();
        p2p.join(space1.clone(), a3.clone()).await.unwrap();

        let res = p2p
            .notify_multi(actor::NotifyMulti {
                space: space1,
                // this is just a dummy value right now
                basis: Arc::new(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_vec().into()),
                remote_agent_count: Some(42),
                timeout_ms: Some(20),
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

        let (mut p2p, mut evt) = spawn_kitsune_p2p().await.unwrap();

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
                        let _ = respond(Ok(b"echo: test-multi-request".to_vec()));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(space1.clone(), a1.clone()).await.unwrap();
        p2p.join(space1.clone(), a2.clone()).await.unwrap();
        p2p.join(space1.clone(), a3.clone()).await.unwrap();

        let res = p2p
            .rpc_multi(actor::RpcMulti {
                space: space1,
                from_agent: a1,
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

        assert_eq!(2, res.len());
        for r in res {
            let data = String::from_utf8_lossy(&r.response);
            assert_eq!("echo: test-multi-request", &data);
            assert!(r.agent == a2 || r.agent == a3);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }
}
