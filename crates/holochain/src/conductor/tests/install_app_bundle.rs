use std::{collections::HashMap, path::PathBuf};

use crate::{conductor::error::ConductorError, sweettest::*};
use futures::future::join_all;
use holo_hash::DnaHash;
use holochain_types::prelude::{
    AppBundle, AppBundleSource, AppManifestCurrentBuilder, AppRoleDnaManifest, AppRoleManifest,
    CellProvisioning, DnaBundle, DnaLocation, DnaVersionSpec, InstallAppPayload,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{CellId, DnaModifiersOpt};
use matches::assert_matches;

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
    let cell_id = CellId::new(dna.dna_hash().to_owned(), alice.clone());

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

    let resources = vec![(
        path.clone(),
        DnaBundle::from_dna_file(dna.clone()).await.unwrap(),
    )];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let duplicate_install_with_app_disabled = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: HashMap::new(),
            network_seed: None,
        })
        .await;
    assert_matches!(
        duplicate_install_with_app_disabled.unwrap_err(),
        ConductorError::CellAlreadyExists(id) if id == cell_id
    );

    // enable app
    conductor.enable_app("app_1".into()).await.unwrap();

    let resources = vec![(
        path.clone(),
        DnaBundle::from_dna_file(dna.clone()).await.unwrap(),
    )];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let duplicate_install_with_app_enabled = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: HashMap::new(),
            network_seed: None,
        })
        .await;
    assert_matches!(
        duplicate_install_with_app_enabled.unwrap_err(),
        ConductorError::CellAlreadyExists(id) if id == cell_id
    );

    let resources = vec![(path, DnaBundle::from_dna_file(dna.clone()).await.unwrap())];
    let bundle = AppBundle::new(manifest.into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let valid_install_of_second_app = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: HashMap::new(),
            network_seed: Some("network".into()),
        })
        .await;
    assert!(valid_install_of_second_app.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn network_seed_affects_dna_hash_when_app_bundle_is_installed() {
    let conductor = SweetConductor::from_standard_config().await;
    let agents = SweetAgents::get(conductor.keystore(), 4).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let path = PathBuf::from(format!("{}", dna.dna_hash()));
    let modifiers = DnaModifiersOpt::none().with_network_seed("builtin-seed".to_string());
    let dnas = vec![dna.dna_def().clone()];

    let bundle = || async {
        let hashes = join_all(
            dnas.clone()
                .into_iter()
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

        AppBundle::new(manifest.into(), resources, PathBuf::from("."))
            .await
            .unwrap()
    };

    let app0 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: agents[0].clone(),
            source: AppBundleSource::Bundle(bundle().await),
            installed_app_id: Some("app_0".into()),
            network_seed: None,
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();

    let app1 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: agents[1].clone(),
            source: AppBundleSource::Bundle(bundle().await),
            installed_app_id: Some("app_1".into()),
            network_seed: Some("same-seed".to_string()),
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();

    let app2 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: agents[2].clone(),
            source: AppBundleSource::Bundle(bundle().await),
            installed_app_id: Some("app_2".into()),
            network_seed: Some("same-seed".to_string()),
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();

    let app3 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: agents[3].clone(),
            source: AppBundleSource::Bundle(bundle().await),
            installed_app_id: Some("app_3".into()),
            network_seed: Some("different-seed".to_string()),
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();

    assert_ne!(
        app0.all_cells().next().unwrap().dna_hash(),
        app1.all_cells().next().unwrap().dna_hash()
    );
    assert_eq!(
        app1.all_cells().next().unwrap().dna_hash(),
        app2.all_cells().next().unwrap().dna_hash()
    );
    assert_ne!(
        app2.all_cells().next().unwrap().dna_hash(),
        app3.all_cells().next().unwrap().dna_hash()
    );
}
