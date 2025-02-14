use ::fixt::fixt;
use hdk::prelude::{ChainTopOrdering, CreateLinkInput, DeleteLinkInput, LinkType};
use holo_hash::{fixt::AgentPubKeyFixturator, ActionHash};
use holochain::sweettest::{
    await_consistency, SweetConductorBatch, SweetConductorConfig, SweetDnaFile, SweetInlineZomes,
};
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;

#[tokio::test(flavor = "multi_thread")]
async fn delete_link() {
    let zomes = SweetInlineZomes::new(vec![], 1)
        .function("create_some_link", |host_api, ()| {
            let base_address = fixt!(AgentPubKey);
            let target_address = fixt!(AgentPubKey);
            let input = CreateLinkInput::new(
                base_address.into(),
                target_address.into(),
                0.into(),
                LinkType::new(0),
                "".into(),
                ChainTopOrdering::Relaxed,
            );
            let action_hash = host_api.create_link(input).unwrap();
            Ok(action_hash)
        })
        .function("delete_that_link", |host_api, link_address: ActionHash| {
            let input = DeleteLinkInput::new(link_address, ChainTopOrdering::Relaxed);
            let action_hash = host_api.delete_link(input).unwrap();
            Ok(action_hash)
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    let kparams = KitsuneP2pTuningParams::default();
    // kparams.gossip_arc_clamping = "empty".to_string();
    println!("arc clamping is {:?}", kparams.arc_clamping());
    let config = SweetConductorConfig::rendezvous(true).set_tuning_params(kparams);

    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
    let apps = conductors.setup_app("", &[dna_file.clone()]).await.unwrap();

    await_consistency(20, &apps.cells_flattened())
        .await
        .unwrap();

    let alice_zome = apps[0].cells()[0].zome(SweetInlineZomes::COORDINATOR);
    let link_create_hash: ActionHash = conductors[0]
        .call(&alice_zome, "create_some_link", ())
        .await;
    println!("link create hash {link_create_hash:?}");

    // await_consistency(10, &apps.cells_flattened())
    //     .await
    //     .unwrap();

    let bob_zome = apps[1].cells()[0].zome(SweetInlineZomes::COORDINATOR);
    let link_delete_hash: ActionHash = conductors[1]
        .call(&bob_zome, "delete_that_link", link_create_hash)
        .await;
    println!("link delete hash {link_delete_hash:?}");
}
