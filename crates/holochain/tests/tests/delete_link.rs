use ::fixt::fixt;
use hdk::{
    link::GetLinksInputBuilder,
    prelude::{ChainTopOrdering, CreateLinkInput, DeleteLinkInput, Link, LinkTypeFilter},
};
use holo_hash::{fixt::AgentPubKeyFixturator, ActionHash, AnyLinkableHash};
use holochain::{
    prelude::DnaFile,
    sweettest::{
        SweetConductor, SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
        SweetLocalRendezvous,
    },
};
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
use std::time::Duration;

async fn create_dna() -> DnaFile {
    let zomes = SweetInlineZomes::new(vec![], 1)
        .function(
            "create_some_link",
            move |host_api, link_base_address: AnyLinkableHash| {
                let target_address = fixt!(AgentPubKey);
                let input = CreateLinkInput::new(
                    link_base_address,
                    target_address.into(),
                    0.into(),
                    0.into(),
                    "".into(),
                    ChainTopOrdering::Relaxed,
                );
                let action_hash = host_api.create_link(input).unwrap();
                Ok(action_hash)
            },
        )
        .function(
            "get_all_links",
            move |host_api, base_address: AnyLinkableHash| {
                let input = GetLinksInputBuilder::try_new(
                    base_address,
                    LinkTypeFilter::Types(vec![(0.into(), vec![0.into()])]),
                )
                .unwrap()
                .build();
                let links = host_api
                    .get_links(vec![input])
                    .unwrap()
                    .first()
                    .unwrap()
                    .to_owned();
                Ok(links)
            },
        )
        .function("delete_that_link", |host_api, link_address: ActionHash| {
            let input = DeleteLinkInput::new(link_address, ChainTopOrdering::Relaxed);
            let action_hash = host_api.delete_link(input).unwrap();
            Ok(action_hash)
        });
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    dna_file
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links_and_delete_link() {
    let dna_file = create_dna().await;

    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
    let apps = conductors.setup_app("", &[dna_file.clone()]).await.unwrap();
    let conductor_alice = &conductors[0];
    let conductor_bob = &conductors[1];
    let alice_app = &apps[0];
    let bob_app = &apps[1];

    conductor_bob
        .wait_for_peer_visible([alice_app.agent().clone()], None, Duration::from_secs(5))
        .await
        .unwrap();

    // Alice creates link.
    let alice_zome = alice_app.cells()[0].zome(SweetInlineZomes::COORDINATOR);
    let link_base_address = AnyLinkableHash::from(fixt!(AgentPubKey));
    let link_create_hash: ActionHash = conductor_alice
        .call(&alice_zome, "create_some_link", link_base_address.clone())
        .await;

    // Bob gets all links and deletes the created link.
    let bob_zome = bob_app.cells()[0].zome(SweetInlineZomes::COORDINATOR);
    let all_links: Vec<Link> = conductor_bob
        .call(&bob_zome, "get_all_links", link_base_address.clone())
        .await;
    assert_eq!(all_links.len(), 1);

    let _: ActionHash = conductor_bob
        .call(&bob_zome, "delete_that_link", link_create_hash)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links_and_delete_link_with_empty_arc() {
    let dna_file = create_dna().await;

    let config = SweetConductorConfig::rendezvous(true);
    let rendezvous = SweetLocalRendezvous::new().await;
    let mut conductor_alice =
        SweetConductor::from_config_rendezvous(config.clone(), rendezvous.clone()).await;
    let alice_app = conductor_alice
        .setup_app("", &[dna_file.clone()])
        .await
        .unwrap();

    // Clamp Bob's arc to "empty".
    let mut kparams = KitsuneP2pTuningParams::default();
    kparams.gossip_arc_clamping = "empty".to_string();
    let bob_config = config.set_tuning_params(kparams);
    let mut conductor_bob = SweetConductor::from_config_rendezvous(bob_config, rendezvous).await;
    let bob_app = conductor_bob
        .setup_app("", &[dna_file.clone()])
        .await
        .unwrap();

    conductor_bob
        .wait_for_peer_visible([alice_app.agent().clone()], None, Duration::from_secs(5))
        .await
        .unwrap();

    // Alice with full arc creates a link.
    let alice_zome = alice_app.cells()[0].zome(SweetInlineZomes::COORDINATOR);
    let link_base_address = AnyLinkableHash::from(fixt!(AgentPubKey));
    let link_create_hash: ActionHash = conductor_alice
        .call(&alice_zome, "create_some_link", link_base_address.clone())
        .await;

    // Bob with empty arc gets all links and deletes the created link.
    let bob_zome = bob_app.cells()[0].zome(SweetInlineZomes::COORDINATOR);
    let all_links: Vec<Link> = conductor_bob
        .call(&bob_zome, "get_all_links", link_base_address)
        .await;
    assert_eq!(all_links.len(), 1);

    let _: ActionHash = conductor_bob
        .call(&bob_zome, "delete_that_link", link_create_hash)
        .await;
}
