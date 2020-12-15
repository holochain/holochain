use futures::future;
use hdk3::prelude::*;
use holochain::{
    conductor::{config::ConductorConfig, p2p_store::exchange_peer_info, Conductor},
    destructure_test_cells,
};
use holochain::{
    destructure_test_cell_vec,
    test_utils::cool::{CoolAgents, CoolConductorHandle, MaybeElement},
};
use holochain_state::test_utils::test_environments;
use holochain_types::dna::{zome::inline_zome::InlineZome, DnaFile};
use holochain_zome_types::element::ElementEntry;
use kitsune_p2p::KitsuneP2pConfig;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

fn simple_crud_zome() -> InlineZome {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create((entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get((hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

// TODO [ B-03669 ]: make much less verbose
#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn multi_conductor() -> anyhow::Result<()> {
    const NUM_CONDUCTORS: usize = 3;

    let conductors = future::join_all(
        std::iter::repeat_with(|| async move {
            let envs = test_environments();
            let mut network = KitsuneP2pConfig::default();
            network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
                bind_to: None,
                override_host: None,
                override_port: None,
            }];
            let conductor: CoolConductorHandle = Conductor::builder()
                .config(ConductorConfig {
                    network: Some(network),
                    ..Default::default()
                })
                .test(&envs)
                .await
                .unwrap()
                .into();
            (conductor, envs)
        })
        .take(NUM_CONDUCTORS),
    )
    .await;

    let (dna_file, _) = DnaFile::unique_from_inline_zome("zome1", simple_crud_zome())
        .await
        .unwrap();

    let data = future::join_all(conductors.iter().map(|(conductor, envs)| {
        let dna_file = dna_file.clone();
        async move {
            let data = conductor
                .setup_app_for_agents_with_no_membrane_proof(
                    "app",
                    &[CoolAgents::one(envs.keystore()).await],
                    &[dna_file.clone()],
                )
                .await;
            (data, envs)
        }
    }))
    .await;

    let p2p_envs = data.iter().map(|(_, envs)| envs.p2p()).collect();
    exchange_peer_info(p2p_envs);

    let (((alice,),), ((bobbo,),), ((_carol,),)) =
        destructure_test_cell_vec!(data.into_iter().map(|(data, _)| data).collect());

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = alice.call("zome1", "create", ()).await;

    // Wait long enough for Bob to receive gossip (TODO: make deterministic)
    tokio::time::delay_for(std::time::Duration::from_millis(5000)).await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: MaybeElement = bobbo.call("zome1", "read", hash).await;
    let element = element
        .0
        .expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}
