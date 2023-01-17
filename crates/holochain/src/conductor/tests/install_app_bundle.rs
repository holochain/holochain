use std::{collections::HashMap, path::PathBuf};

use crate::sweettest::*;
use futures::future::join_all;
use holo_hash::DnaHash;
use holochain_types::prelude::{
    AppBundle, AppBundleSource, AppManifestCurrentBuilder, AppRoleDnaManifest, AppRoleManifest,
    CellProvisioning, DnaBundle, DnaLocation, DnaVersionSpec, InstallAppPayload,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::DnaModifiersOpt;

#[tokio::test(flavor = "multi_thread")]
async fn reject_duplicate_app_for_same_agent() {
    let conductor = SweetConductor::from_standard_config().await;
    let alice = SweetAgents::one(conductor.keystore()).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let path = PathBuf::from(format!("{}", dna.dna_hash()));
    let modifiers = DnaModifiersOpt::none();
    let dnas = vec![dna.dna_def().clone()];
    let hashes = join_all(
        dnas.into_iter()
            .map(|dna| async move { DnaHash::with_data_sync(&dna).into() }),
    )
    .await;

    let version = DnaVersionSpec::from(hashes.clone()).into();

    let roles = vec![AppRoleManifest {
        name: "name".into(),
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Bundled(path.clone())),
            modifiers: modifiers.clone(),
            version: Some(version),
            clone_limit: 0,
        },
        provisioning: Some(CellProvisioning::Create { deferred: false }),
    }];

    let manifest = AppManifestCurrentBuilder::default()
        .name("test_app".into())
        .description(None)
        .roles(roles)
        .build()
        .unwrap();
    let resources = vec![(
        path.clone(),
        DnaBundle::from_dna_file(dna.clone()).await.unwrap(),
    )];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();

    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: alice.clone(),
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();

    let resources = vec![(path, DnaBundle::from_dna_file(dna.clone()).await.unwrap())];
    let bundle = AppBundle::new(manifest.into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let duplicate_install = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: HashMap::new(),
            network_seed: None,
        })
        .await;
    println!("duplicate install {:?}", duplicate_install);
    assert!(duplicate_install.is_err());
}
