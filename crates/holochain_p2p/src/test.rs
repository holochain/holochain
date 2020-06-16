#[cfg(test)]
mod tests {
    use crate::*;

    fn fake_dht_op() -> holochain_types::dht_op::DhtOp {
        let a1: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"111111111111111111111111111111111111".to_vec(),
        )
        .into();
        let d1: holo_hash::DnaHash =
            crate::holo_hash_core::DnaHash::new(b"ssssssssssssssssssssssssssssssssssss".to_vec())
                .into();
        holochain_types::dht_op::DhtOp::StoreElement(
            holochain_keystore::Signature(vec![0; 32]),
            holochain_types::header::Header::Dna(holochain_types::header::Dna {
                author: a1,
                timestamp: holochain_types::Timestamp::now(),
                header_seq: 0,
                hash: d1,
            }),
            None,
        )
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_call_remote_workflow() {
        let dna: holo_hash::DnaHash =
            crate::holo_hash_core::DnaHash::new(b"ssssssssssssssssssssssssssssssssssss".to_vec())
                .into();
        let a1: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"111111111111111111111111111111111111".to_vec(),
        )
        .into();
        let a2: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"222222222222222222222222222222222222".to_vec(),
        )
        .into();

        let (mut p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    CallRemote { respond, .. } => {
                        let _ = respond(Ok(UnsafeBytes::from(b"yada".to_vec()).into()));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let res = p2p
            .call_remote(dna, a2, UnsafeBytes::from(b"yippo".to_vec()).into())
            .await
            .unwrap();
        let res: Vec<u8> = UnsafeBytes::from(res).into();

        assert_eq!(b"yada".to_vec(), res);

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_send_validation_receipt_workflow() {
        let dna: holo_hash::DnaHash =
            crate::holo_hash_core::DnaHash::new(b"ssssssssssssssssssssssssssssssssssss".to_vec())
                .into();
        let a1: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"111111111111111111111111111111111111".to_vec(),
        )
        .into();
        let a2: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"222222222222222222222222222222222222".to_vec(),
        )
        .into();

        let (mut p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    ValidationReceiptReceived {
                        respond, receipt, ..
                    } => {
                        let receipt: Vec<u8> = UnsafeBytes::from(receipt).into();
                        assert_eq!(b"receipt-test".to_vec(), receipt);
                        let _ = respond(Ok(()));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        p2p.send_validation_receipt(dna, a2, UnsafeBytes::from(b"receipt-test".to_vec()).into())
            .await
            .unwrap();

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_publish_workflow() {
        let dna: holo_hash::DnaHash =
            crate::holo_hash_core::DnaHash::new(b"ssssssssssssssssssssssssssssssssssss".to_vec())
                .into();
        let a1: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"111111111111111111111111111111111111".to_vec(),
        )
        .into();
        let a2: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"222222222222222222222222222222222222".to_vec(),
        )
        .into();
        let a3: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"333333333333333333333333333333333333".to_vec(),
        )
        .into();

        let (mut p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let recv_count = Arc::new(std::sync::atomic::AtomicU8::new(0));

        let recv_count_clone = recv_count.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Publish { respond, .. } => {
                        let _ = respond(Ok(()));
                        recv_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();
        p2p.join(dna.clone(), a3.clone()).await.unwrap();

        let entry_hash = holochain_types::composite_hash::AnyDhtHash::from(
            holo_hash::EntryContentHash::from(crate::holo_hash_core::EntryContentHash::new(
                b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            )),
        );

        p2p.publish(dna, a1, true, entry_hash, vec![], Some(20))
            .await
            .unwrap();

        assert_eq!(3, recv_count.load(std::sync::atomic::Ordering::SeqCst));

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_get_workflow() {
        let dna: holo_hash::DnaHash =
            crate::holo_hash_core::DnaHash::new(b"ssssssssssssssssssssssssssssssssssss".to_vec())
                .into();
        let a1: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"111111111111111111111111111111111111".to_vec(),
        )
        .into();
        let a2: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"222222222222222222222222222222222222".to_vec(),
        )
        .into();
        let a3: holo_hash::AgentPubKey = crate::holo_hash_core::AgentPubKey::new(
            b"333333333333333333333333333333333333".to_vec(),
        )
        .into();

        let dht_op_hash_1: holo_hash::DhtOpHash =
            crate::holo_hash_core::DhtOpHash::new(b"444444444444444444444444444444444444".to_vec())
                .into();
        let dht_op_hash_2: holo_hash::DhtOpHash =
            crate::holo_hash_core::DhtOpHash::new(b"555555555555555555555555555555555555".to_vec())
                .into();

        let (mut p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let mut respond_queue = vec![
            dht_op_hash_1.clone(),
            dht_op_hash_2.clone(),
            dht_op_hash_1.clone(),
        ];
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Get { respond, .. } => {
                        let mut out = Vec::new();
                        for _ in 0..2 {
                            if let Some(h) = respond_queue.pop() {
                                out.push((h, fake_dht_op()));
                            }
                        }
                        let _ = respond(Ok(out));
                    }
                    _ => panic!("unexpected event"),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();
        p2p.join(dna.clone(), a3.clone()).await.unwrap();

        let entry_hash = holochain_types::composite_hash::AnyDhtHash::from(
            holo_hash::EntryContentHash::from(crate::holo_hash_core::EntryContentHash::new(
                b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            )),
        );

        let res = p2p
            .get(dna, a1, entry_hash, actor::GetOptions::default())
            .await
            .unwrap();

        assert_eq!(2, res.len());

        for r in res {
            assert!(r.0 == dht_op_hash_1 || r.0 == dht_op_hash_2);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }
}
