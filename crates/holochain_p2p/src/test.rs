#[cfg(test)]
mod tests {
    use crate::*;

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

        tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    CallRemote {
                        respond,
                        dna_hash: _,
                        agent_pub_key: _,
                        request: _,
                        ..
                    } => {
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
            b"111111111111111111111111111111111111".to_vec(),
        )
        .into();

        let (mut p2p, mut evt) = spawn_holochain_p2p().await.unwrap();

        tokio::task::spawn(async move {
            use tokio::stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    SendValidationReceipt {
                        respond,
                        dna_hash: _,
                        agent_pub_key: _,
                        receipt,
                        ..
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
    }
}
