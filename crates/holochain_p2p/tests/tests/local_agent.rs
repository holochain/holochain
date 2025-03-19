use bytes::Bytes;
use holochain_keystore::{test_keystore, AgentPubKeyExt};
use holochain_p2p::HolochainP2pLocalAgent;
use holochain_types::prelude::{Signature, SIGNATURE_BYTES};
use kitsune2_api::{AgentId, AgentInfo, DhtArc, LocalAgent, Signer, SpaceId, Timestamp};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn create_and_sign() {
    let client = test_keystore();
    let agent = client.new_sign_keypair_random().await.unwrap();

    let local_agent = HolochainP2pLocalAgent::new(agent.clone(), DhtArc::FULL, 1, client);

    // Check initial arc values
    assert_eq!(DhtArc::FULL, local_agent.get_tgt_storage_arc());
    assert_eq!(DhtArc::Empty, local_agent.get_cur_storage_arc());

    // Set the current arc
    local_agent.set_cur_storage_arc(DhtArc::FULL);
    assert_eq!(DhtArc::FULL, local_agent.get_cur_storage_arc());

    // Set the target arc
    local_agent.set_tgt_storage_arc_hint(DhtArc::Arc(0, 100));
    assert_eq!(DhtArc::Arc(0, 100), local_agent.get_tgt_storage_arc());

    // Call the callback with no callback set
    local_agent.invoke_cb();

    // Register a callback
    let value = Arc::new(AtomicBool::new(false));
    let cb = {
        let value = value.clone();
        move || {
            value.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    };
    local_agent.register_cb(Arc::new(cb));

    // Invoke the callback
    local_agent.invoke_cb();

    // Check that the callback was called
    assert!(value.load(std::sync::atomic::Ordering::Relaxed));

    // Sign a message
    let message = b"test message";
    let signature = local_agent
        .sign(
            &AgentInfo {
                agent: AgentId::from(Bytes::from_static(b"test agent")),
                space: SpaceId::from(Bytes::from_static(b"test space")),
                created_at: Timestamp::now(),
                expires_at: Timestamp::now() + Duration::from_secs(30),
                is_tombstone: false,
                url: None,
                storage_arc: Default::default(),
            },
            message,
        )
        .await
        .unwrap();

    assert_eq!(SIGNATURE_BYTES, signature.len());

    agent
        .verify_signature_raw(
            &Signature(signature.as_ref().try_into().unwrap()),
            Arc::new(*message),
        )
        .await
        .unwrap();
}
