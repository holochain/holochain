use crate::actor::HolochainP2pRefToCell;
use crate::HolochainP2pCell;
use fixt::prelude::*;
use holo_hash::fixt::{AgentPubKeyFixturator, DnaHashFixturator};

fixturator!(
    HolochainP2pCell;
    curve Empty {
        // TODO: Make this empty
        tokio_safe_block_on::tokio_safe_block_forever_on(async {
            let (holochain_p2p, _p2p_evt) = crate::spawn_holochain_p2p().await.unwrap();
            holochain_p2p.to_cell(
                DnaHashFixturator::new(Empty).next().unwrap(),
                AgentPubKeyFixturator::new(Empty).next().unwrap(),
            )
        })
    };
    curve Unpredictable {
        // TODO: Make this unpredictable
        tokio_safe_block_on::tokio_safe_block_forever_on(async {
            let (holochain_p2p, _p2p_evt) = crate::spawn_holochain_p2p().await.unwrap();
            holochain_p2p.to_cell(
                DnaHashFixturator::new(Unpredictable).next().unwrap(),
                AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
            )
        })
    };
    curve Predictable {
        tokio_safe_block_on::tokio_safe_block_forever_on(async {
            let (holochain_p2p, _p2p_evt) = crate::spawn_holochain_p2p().await.unwrap();
            holochain_p2p.to_cell(
                DnaHashFixturator::new(Predictable).next().unwrap(),
                AgentPubKeyFixturator::new(Predictable).next().unwrap(),
            )
        })
    };
);
#[cfg(test)]
mod tests {
    use crate::*;
    use ::fixt::prelude::*;
    use futures::future::FutureExt;
    use ghost_actor::GhostControlSender;
    use holochain_types::element::{Element, SignedHeaderHashed, WireElement};
    use holochain_types::fixt::*;

    macro_rules! newhash {
        ($p:ident, $c:expr) => {
            holo_hash::$p::from_raw_bytes([$c as u8; 36].to_vec())
        };
    }

    fn test_setup() -> (
        holo_hash::DnaHash,
        holo_hash::AgentPubKey,
        holo_hash::AgentPubKey,
        holo_hash::AgentPubKey,
    ) {
        (
            newhash!(DnaHash, 's'),
            newhash!(AgentPubKey, '1'),
            newhash!(AgentPubKey, '2'),
            newhash!(AgentPubKey, '3'),
        )
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_call_remote_workflow() {
        let (dna, a1, a2, _) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    CallRemote { respond, .. } => {
                        respond.r(Ok(
                            async move { Ok(UnsafeBytes::from(b"yada".to_vec()).into()) }
                                .boxed()
                                .into(),
                        ));
                    }
                    _ => (),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let res = p2p
            .call_remote(
                dna,
                a1,
                a2,
                "".into(),
                "".to_string(),
                "".to_string().into(),
                UnsafeBytes::from(b"yippo".to_vec()).into(),
            )
            .await
            .unwrap();
        let res: Vec<u8> = UnsafeBytes::from(res).into();

        assert_eq!(b"yada".to_vec(), res);

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_send_validation_receipt_workflow() {
        let (dna, a1, a2, _) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

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
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    _ => (),
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
    // @TODO flakey test
    // ---- test::tests::test_publish_workflow stdout ----
    // thread 'test::tests::test_publish_workflow' panicked at 'assertion failed: `(left == right)`
    //   left: `3`,
    //  right: `0`', crates/holochain_p2p/src/test.rs:181:9
    async fn test_publish_workflow() {
        let (dna, a1, a2, a3) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let recv_count = Arc::new(std::sync::atomic::AtomicU8::new(0));

        let recv_count_clone = recv_count.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Publish { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                        recv_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    _ => (),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();
        p2p.join(dna.clone(), a3.clone()).await.unwrap();

        let header_hash = holo_hash::AnyDhtHash::from_raw_bytes_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyDht::Header,
        );

        p2p.publish(dna, a1, true, header_hash, vec![], Some(20))
            .await
            .unwrap();

        assert_eq!(3, recv_count.load(std::sync::atomic::Ordering::SeqCst));

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_get_workflow() {
        let (dna, a1, a2, a3) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let test_1 = GetElementResponse::GetHeader(Some(Box::new(WireElement::from_element(
            Element::new(
                SignedHeaderHashed::with_presigned(
                    HoloHashed::from_content(fixt!(Header)).await,
                    fixt!(Signature),
                ),
                None,
            ),
            None,
        ))));
        let test_2 = GetElementResponse::GetHeader(Some(Box::new(WireElement::from_element(
            Element::new(
                SignedHeaderHashed::with_presigned(
                    HoloHashed::from_content(fixt!(Header)).await,
                    fixt!(Signature),
                ),
                None,
            ),
            None,
        ))));

        let mut respond_queue = vec![test_1.clone(), test_2.clone()];
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Get { respond, .. } => {
                        let resp = if let Some(h) = respond_queue.pop() {
                            h
                        } else {
                            panic!("too many requests!")
                        };
                        respond.r(Ok(async move { Ok(resp) }.boxed().into()));
                    }
                    _ => (),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();
        p2p.join(dna.clone(), a3.clone()).await.unwrap();

        let hash = holo_hash::AnyDhtHash::from_raw_bytes_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyDht::Header,
        );

        let res = p2p
            .get(dna, a1, hash, actor::GetOptions::default())
            .await
            .unwrap();

        assert_eq!(1, res.len());

        for r in res {
            assert!(r == test_1 || r == test_2);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_get_links_workflow() {
        let (dna, a1, a2, _) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        let test_1 = GetLinksResponse {
            link_adds: vec![(fixt!(LinkAdd), fixt!(Signature))],
            link_removes: vec![(fixt!(LinkRemove), fixt!(Signature))],
        };

        let test_1_clone = test_1.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                let test_1_clone = test_1_clone.clone();
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    GetLinks { respond, .. } => {
                        respond.r(Ok(async move { Ok(test_1_clone) }.boxed().into()));
                    }
                    _ => (),
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let hash = holo_hash::EntryHash::from_raw_bytes_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::Entry::Content,
        );
        let link_key = WireLinkMetaKey::Base(hash);

        let res = p2p
            .get_links(dna, a1, link_key, actor::GetLinksOptions::default())
            .await
            .unwrap();

        assert_eq!(1, res.len());

        for r in res {
            assert_eq!(r, test_1);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }
}
