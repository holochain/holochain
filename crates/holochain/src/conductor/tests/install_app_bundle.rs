use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use crate::{conductor::error::ConductorError, sweettest::*};
use ::fixt::prelude::*;
use holo_hash::DnaHash;
use holochain_conductor_api::AppInfoStatus;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use maplit::btreeset;
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
async fn clone_only_provisioning_creates_no_cell_and_allows_cloning() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    async fn make_payload(clone_limit: u32) -> InstallAppPayload {
        // The integrity zome in this WASM will fail if the properties are not set. This helps verify that genesis
        // is not being run for the clone-only cell and will only run for the cloned cells.
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
            TestWasm::GenesisSelfCheckRequiresProperties,
        ])
        .await;
        let path = PathBuf::from(format!("{}", dna.dna_hash()));
        let modifiers = DnaModifiersOpt::none();

        let roles = vec![AppRoleManifest {
            name: "name".into(),
            dna: AppRoleDnaManifest {
                location: Some(DnaLocation::Bundled(path.clone())),
                modifiers: modifiers.clone(),
                installed_hash: None,
                clone_limit,
            },
            provisioning: Some(CellProvisioning::CloneOnly),
        }];

        let manifest = AppManifestCurrentBuilder::default()
            .name("test_app".into())
            .description(None)
            .roles(roles)
            .build()
            .unwrap();
        let dna_bundle = DnaBundle::from_dna_file(dna.clone()).unwrap();
        let resources = vec![(path.clone(), dna_bundle)];
        let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
            .await
            .unwrap();

        InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        }
    }

    // Fails due to clone limit of 0
    assert_matches!(
        conductor
            .clone()
            .install_app_bundle(make_payload(0).await)
            .await
            .unwrap_err(),
        ConductorError::AppBundleError(AppBundleError::AppManifestError(
            AppManifestError::InvalidStrategyCloneOnly(_)
        ))
    );

    {
        // Succeeds with clone limit of 1
        let app = conductor
            .clone()
            .install_app_bundle(make_payload(1).await)
            .await
            .unwrap();

        // No cells in this app due to CloneOnly provisioning strategy
        assert_eq!(app.all_cells().count(), 0);
        assert_eq!(app.role_assignments().len(), 1);
    }
    {
        let clone_cell = conductor
            .create_clone_cell(
                &"app_1".into(),
                CreateCloneCellPayload {
                    role_name: "name".into(),
                    modifiers: DnaModifiersOpt::none()
                        .with_network_seed("1".into())
                        .with_properties(YamlProperties::new(serde_yaml::Value::String(
                            "foo".into(),
                        ))),
                    membrane_proof: None,
                    name: Some("Johnny".into()),
                },
            )
            .await
            .unwrap();

        let state = conductor.get_state().await.unwrap();
        let app = state.get_app(&"app_1".to_string()).unwrap();

        assert_eq!(clone_cell.name, "Johnny".to_string());
        assert_eq!(app.role_assignments().len(), 1);
        assert_eq!(app.clone_cells().count(), 1);
    }
    {
        let err = conductor
            .create_clone_cell(
                &"app_1".into(),
                CreateCloneCellPayload {
                    role_name: "name".into(),
                    modifiers: DnaModifiersOpt::none()
                        .with_network_seed("1".into())
                        .with_properties(YamlProperties::new(serde_yaml::Value::String(
                            "foo".into(),
                        ))),
                    membrane_proof: None,
                    name: None,
                },
            )
            .await
            .unwrap_err();
        assert_matches!(
            err,
            ConductorError::AppError(AppError::CloneLimitExceeded(1, _))
        );
        let state = conductor.get_state().await.unwrap();
        let app = state.get_app(&"app_1".to_string()).unwrap();

        assert_eq!(app.all_cells().count(), 1);
    }
    // TODO: test that the cell can't be provisioned later
}

#[tokio::test(flavor = "multi_thread")]
async fn reject_duplicate_app_for_same_agent() {
    let conductor = SweetConductor::from_standard_config().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let path = PathBuf::from(format!("{}", dna.dna_hash()));
    let modifiers = DnaModifiersOpt::none();

    let roles = vec![AppRoleManifest {
        name: "name".into(),
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Bundled(path.clone())),
            modifiers: modifiers.clone(),
            installed_hash: None,
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
    let resources = vec![(path.clone(), DnaBundle::from_dna_file(dna.clone()).unwrap())];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();

    let app = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();
    let alice = app.agent_key().clone();

    let cell_id = CellId::new(dna.dna_hash().to_owned(), app.agent_key().clone());

    let resources = vec![(path.clone(), DnaBundle::from_dna_file(dna.clone()).unwrap())];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let duplicate_install_with_app_disabled = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: Some(alice.clone()),
            installed_app_id: Some("app_2".into()),
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
            network_seed: None,
        })
        .await;
    assert_matches!(
        duplicate_install_with_app_disabled.unwrap_err(),
        ConductorError::CellAlreadyExists(id) if id == cell_id
    );

    // enable app
    conductor.enable_app("app_1".into()).await.unwrap();

    let resources = vec![(path.clone(), DnaBundle::from_dna_file(dna.clone()).unwrap())];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let duplicate_install_with_app_enabled = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: Some(alice.clone()),
            installed_app_id: Some("app_2".into()),
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
            network_seed: None,
        })
        .await;
    assert_matches!(
        duplicate_install_with_app_enabled.unwrap_err(),
        ConductorError::CellAlreadyExists(id) if id == cell_id
    );

    let resources = vec![(path, DnaBundle::from_dna_file(dna.clone()).unwrap())];
    let bundle = AppBundle::new(manifest.into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let valid_install_of_second_app = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: Some(alice.clone()),
            installed_app_id: Some("app_2".into()),
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
            network_seed: Some("network".into()),
        })
        .await;
    assert!(valid_install_of_second_app.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn can_install_app_a_second_time_using_nothing_but_the_manifest_from_app_info() {
    let conductor = SweetConductor::from_standard_config().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let path = PathBuf::from(format!("{}", dna.dna_hash()));
    let modifiers = DnaModifiersOpt::default()
        .with_network_seed("initial seed".into())
        .with_origin_time(Timestamp::now());

    let roles = vec![AppRoleManifest {
        name: "name".into(),
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Bundled(path.clone())),
            modifiers: modifiers.clone(),
            // Note that there is no installed hash provided. We'll check that this changes later.
            installed_hash: None,
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

    let resources = vec![(path.clone(), DnaBundle::from_dna_file(dna.clone()).unwrap())];

    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();

    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: Some("final seed".into()),
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();

    let manifest = conductor
        .get_app_info(&"app_1".to_string())
        .await
        .unwrap()
        .unwrap()
        .manifest;

    let installed_dna = dna.update_modifiers(
        modifiers
            .with_network_seed("final seed".into())
            .serialized()
            .unwrap(),
    );
    let installed_dna_hash = DnaHash::with_data_sync(installed_dna.dna_def());

    // Check that the returned manifest has the installed DNA hash properly set
    assert_eq!(
        manifest.app_roles()[0].dna.installed_hash,
        Some(installed_dna_hash.into())
    );

    assert_eq!(
        manifest.app_roles()[0].dna.modifiers.network_seed,
        Some("final seed".into())
    );

    let bundle = AppBundle::new(manifest, vec![], PathBuf::from("."))
        .await
        .unwrap();

    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_2".into()),
            network_seed: None,
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn can_install_app_with_custom_modifiers_correctly() {
    let conductor = SweetConductor::from_standard_config().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let path = PathBuf::from(format!("{}", dna.dna_hash()));

    let manifest_network_seed = String::from("initial seed from the manifest");
    let manifest_properties = YamlProperties::new(serde_yaml::Value::String(String::from(
        "some properties in the manifest",
    )));
    let manifest_origin_time = Timestamp::now().saturating_sub(&std::time::Duration::from_secs(1));
    let manifest_quantum_time = std::time::Duration::from_secs(1 * 60);

    let modifiers = DnaModifiersOpt::default()
        .with_network_seed(manifest_network_seed.clone())
        .with_properties(manifest_properties.clone())
        .with_origin_time(manifest_origin_time.clone())
        .with_quantum_time(manifest_quantum_time.clone());

    let role_name_1 = String::from("role1");
    let role_name_2 = String::from("role2");

    let roles = vec![
        AppRoleManifest {
            name: role_name_1.clone(),
            dna: AppRoleDnaManifest {
                location: Some(DnaLocation::Bundled(path.clone())),
                modifiers: modifiers.clone(),
                // Note that there is no installed hash provided. We'll check that this changes later.
                installed_hash: None,
                clone_limit: 0,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        },
        AppRoleManifest {
            name: role_name_2.clone(),
            dna: AppRoleDnaManifest {
                location: Some(DnaLocation::Bundled(path.clone())),
                modifiers: modifiers.clone(),
                // Note that there is no installed hash provided. We'll check that this changes later.
                installed_hash: None,
                clone_limit: 0,
            },
            provisioning: Some(CellProvisioning::Create { deferred: false }),
        },
    ];

    let manifest = AppManifestCurrentBuilder::default()
        .name("test_app".into())
        .description(None)
        .roles(roles)
        .build()
        .unwrap();

    let resources = vec![(path.clone(), DnaBundle::from_dna_file(dna.clone()).unwrap())];

    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();

    //- Test that installing with custom modifiers correctly overwrites the values and that the dna hash
    //  differs from the dna hash when installed without custom modifiers
    let custom_network_seed = String::from("modified seed");
    let custom_properties = YamlProperties::new(serde_yaml::Value::String(String::from(
        "some properties provided at install time",
    )));
    let custom_origin_time = Timestamp::now();
    let custom_quantum_time = std::time::Duration::from_secs(5 * 60);

    let custom_modifiers = DnaModifiersOpt::default()
        .with_network_seed(custom_network_seed.clone())
        .with_origin_time(custom_origin_time)
        .with_quantum_time(custom_quantum_time)
        .with_properties(custom_properties.clone());

    let role_settings = (
        role_name_1.clone(),
        RoleSettings::Provisioned {
            membrane_proof: Default::default(),
            modifiers: Some(custom_modifiers),
        },
    );

    let network_seed_override = "overridden by network_seed field";

    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle.clone()),
            installed_app_id: Some("app_0".into()),
            network_seed: Some(network_seed_override.into()),
            roles_settings: None,
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();

    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle.clone()),
            installed_app_id: Some("app_1".into()),
            network_seed: Some(network_seed_override.into()),
            roles_settings: Some(HashMap::from([role_settings])),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();

    // - Check that the dna hash differs between the app installed with and the one installed without
    //   custom modifiers

    let app_info_0 = conductor
        .get_app_info(&"app_0".to_string())
        .await
        .unwrap()
        .unwrap();

    let dna_hash_0 = app_info_0.cells_for_role(&role_name_1).unwrap()[0]
        .clone()
        .cell_id()
        .unwrap()
        .dna_hash()
        .clone();

    let app_info_1 = conductor
        .get_app_info(&"app_1".to_string())
        .await
        .unwrap()
        .unwrap();

    let dna_hash_1 = app_info_1.clone().cells_for_role(&role_name_1).unwrap()[0]
        .clone()
        .cell_id()
        .unwrap()
        .dna_hash()
        .clone();

    assert_ne!(dna_hash_0, dna_hash_1);

    let manifest = app_info_1.manifest;

    // - Check that the modifers have been set correctly and only for the specified role
    let installed_app_role_1 = manifest
        .app_roles()
        .into_iter()
        .find(|r| &r.name == &role_name_1)
        .unwrap();

    let installed_app_role_2 = manifest
        .app_roles()
        .into_iter()
        .find(|r| &r.name == &role_name_2)
        .unwrap();

    assert_eq!(
        installed_app_role_1.dna.modifiers.network_seed,
        Some(custom_network_seed)
    );
    assert_eq!(
        installed_app_role_1.dna.modifiers.properties,
        Some(custom_properties)
    );
    assert_eq!(
        installed_app_role_1.dna.modifiers.origin_time,
        Some(custom_origin_time)
    );
    assert_eq!(
        installed_app_role_1.dna.modifiers.quantum_time,
        Some(custom_quantum_time)
    );

    assert_eq!(
        installed_app_role_2.dna.modifiers.network_seed,
        Some(network_seed_override.into())
    );
    assert_eq!(
        installed_app_role_2.dna.modifiers.properties,
        Some(manifest_properties.clone())
    );
    assert_eq!(
        installed_app_role_2.dna.modifiers.origin_time,
        Some(manifest_origin_time.clone())
    );
    assert_eq!(
        installed_app_role_2.dna.modifiers.quantum_time,
        Some(manifest_quantum_time.clone())
    );

    //- Test that modifier fields that are None in the modifiers map do not overwrite existing
    //  modifiers from the manifest
    let custom_modifiers = DnaModifiersOpt::default();

    let role_settings = (
        role_name_1.clone(),
        RoleSettings::Provisioned {
            membrane_proof: Default::default(),
            modifiers: Some(custom_modifiers.clone()),
        },
    );

    conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle.clone()),
            installed_app_id: Some("app_2".into()),
            network_seed: None,
            roles_settings: Some(HashMap::from([role_settings])),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();

    let manifest = conductor
        .get_app_info(&"app_2".to_string())
        .await
        .unwrap()
        .unwrap()
        .manifest;

    let installed_app_role_1 = manifest
        .app_roles()
        .into_iter()
        .find(|r| &r.name == &role_name_1)
        .unwrap();

    // Check that the modifers have been set correctly
    assert_eq!(
        installed_app_role_1.dna.modifiers.network_seed,
        Some(manifest_network_seed.clone())
    );
    assert_eq!(
        installed_app_role_1.dna.modifiers.properties,
        Some(manifest_properties.clone())
    );
    assert_eq!(
        installed_app_role_1.dna.modifiers.origin_time,
        Some(manifest_origin_time)
    );
    assert_eq!(
        installed_app_role_1.dna.modifiers.quantum_time,
        Some(manifest_quantum_time)
    );

    //- Check that installing with modifiers for a non-existent role fails
    let role_settings = (
        "unknown role name".into(),
        RoleSettings::Provisioned {
            membrane_proof: Default::default(),
            modifiers: Some(custom_modifiers.clone()),
        },
    );

    let result = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle.clone()),
            installed_app_id: Some("app_3".into()),
            network_seed: Some("final seed".into()),
            roles_settings: Some(HashMap::from([role_settings])),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await;

    assert!(result.is_err());

    //- Check that if providing a membrane proof in the role settings for an app with `allow_deferred_memproofs`
    //  set to `true` in the app manifest, membrane proofs are not deferred and the app has
    //  AppInfoStatus::Running after installation
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Foo]).await;
    let app_id = "app-id".to_string();
    let role_name = "role".to_string();
    let bundle = app_bundle_from_dnas(&[(role_name.clone(), dna)], true, None).await;

    let role_settings = (
        role_name,
        RoleSettings::Provisioned {
            membrane_proof: Some(MembraneProof::new(fixt!(SerializedBytes))),
            modifiers: Some(custom_modifiers.clone()),
        },
    );

    //- Install with a membrane proof provided in the roles_settings
    let app = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            roles_settings: Some(HashMap::from([role_settings])),
            network_seed: None,
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();
    assert_eq!(app.role_assignments().len(), 1);

    //- Status is now Disabled with the normal `NeverStarted` reason.
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(
        app_info.status,
        AppInfoStatus::Disabled {
            reason: DisabledAppReason::NeverStarted
        }
    );

    conductor.enable_app(app_id.clone()).await.unwrap();

    //- Status is Running, i.e. membrane proof provisioning has not been deferred
    let app_info = conductor.get_app_info(&app_id).await.unwrap().unwrap();
    assert_eq!(app_info.status, AppInfoStatus::Running);
}

#[tokio::test(flavor = "multi_thread")]
async fn cells_by_dna_lineage() {
    let mut conductor = SweetConductor::from_standard_config().await;

    async fn mk_dna(lineage: &[&DnaHash]) -> DnaFile {
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let (def, code) = dna.into_parts();
        let mut def = def.into_content();
        def.lineage = lineage.iter().map(|h| (**h).to_owned()).collect();
        DnaFile::from_parts(def.into_hashed(), code)
    }

    // The lineage of a DNA includes the DNA itself
    let dna1 = mk_dna(&[]).await;
    let dna2 = mk_dna(&[dna1.dna_hash()]).await;
    let dna3 = mk_dna(&[dna1.dna_hash(), dna2.dna_hash()]).await;
    // dna1 is removed from the lineage
    let dna4 = mk_dna(&[dna2.dna_hash(), dna3.dna_hash()]).await;
    let dnax = mk_dna(&[]).await;

    let app1 = conductor.setup_app("app1", [&dna1, &dnax]).await.unwrap();
    let app2 = conductor.setup_app("app2", [&dna2]).await.unwrap();
    let app3 = conductor.setup_app("app3", [&dna3]).await.unwrap();
    let app4 = conductor.setup_app("app4", [&dna4]).await.unwrap();

    let lin1 = conductor
        .cells_by_dna_lineage(dna1.dna_hash())
        .await
        .unwrap();
    let lin2 = conductor
        .cells_by_dna_lineage(dna2.dna_hash())
        .await
        .unwrap();
    let lin3 = conductor
        .cells_by_dna_lineage(dna3.dna_hash())
        .await
        .unwrap();
    let lin4 = conductor
        .cells_by_dna_lineage(dna4.dna_hash())
        .await
        .unwrap();
    let linx = conductor
        .cells_by_dna_lineage(dnax.dna_hash())
        .await
        .unwrap();

    fn app_cells(app: &SweetApp, indices: &[usize]) -> (String, BTreeSet<CellId>) {
        (
            app.installed_app_id().clone(),
            indices
                .iter()
                .map(|i| app.cells()[*i].cell_id().clone())
                .collect(),
        )
    }

    pretty_assertions::assert_eq!(
        lin1,
        btreeset![
            app_cells(&app1, &[0]),
            app_cells(&app2, &[0]),
            app_cells(&app3, &[0]),
            // no dna4: dna1 was "removed"
        ]
    );
    pretty_assertions::assert_eq!(
        lin2,
        btreeset![
            // no dna1: it's in the past
            app_cells(&app2, &[0]),
            app_cells(&app3, &[0]),
            app_cells(&app4, &[0]),
        ]
    );
    pretty_assertions::assert_eq!(
        lin3,
        btreeset![
            // no dna1 or dna2: they're in the past
            app_cells(&app3, &[0]),
            app_cells(&app4, &[0]),
        ]
    );
    pretty_assertions::assert_eq!(
        lin4,
        btreeset![
            // all other dnas are in the past
            app_cells(&app4, &[0]),
        ]
    );
    pretty_assertions::assert_eq!(linx, btreeset![app_cells(&app1, &[1]),]);
}

#[tokio::test(flavor = "multi_thread")]
async fn use_existing_integration() {
    let conductor = SweetConductor::from_standard_config().await;

    let (dna1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::WhoAmI]).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::WhoAmI]).await;

    let bundle1 = {
        let path = PathBuf::from(format!("{}", dna1.dna_hash()));

        let roles = vec![AppRoleManifest {
            name: "created".into(),
            dna: AppRoleDnaManifest {
                location: Some(DnaLocation::Bundled(path.clone())),
                modifiers: DnaModifiersOpt::none(),
                installed_hash: None,
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
            DnaBundle::from_dna_file(dna1.clone()).unwrap(),
        )];
        AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
            .await
            .unwrap()
    };

    let bundle2 = |correct: bool| {
        let dna2 = dna2.clone();
        async move {
            let path = PathBuf::from(format!("{}", dna2.dna_hash()));
            let installed_hash = if correct {
                Some(dna2.dna_hash().clone().into())
            } else {
                None
            };

            let roles = vec![
                AppRoleManifest {
                    name: "created".into(),
                    dna: AppRoleDnaManifest {
                        location: Some(DnaLocation::Bundled(path.clone())),
                        modifiers: DnaModifiersOpt::none(),
                        installed_hash: None,
                        clone_limit: 0,
                    },
                    provisioning: Some(CellProvisioning::Create { deferred: false }),
                },
                AppRoleManifest {
                    name: "extant".into(),
                    dna: AppRoleDnaManifest {
                        location: None,
                        modifiers: DnaModifiersOpt::none(),
                        installed_hash,
                        clone_limit: 0,
                    },
                    provisioning: Some(CellProvisioning::UseExisting { protected: true }),
                },
            ];

            let manifest = AppManifestCurrentBuilder::default()
                .name("test_app".into())
                .description(None)
                .roles(roles)
                .build()
                .unwrap();

            let resources = vec![(
                path.clone(),
                DnaBundle::from_dna_file(dna2.clone()).unwrap(),
            )];
            AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
                .await
                .unwrap()
        }
    };

    // Install the "dependency" app
    let app_1 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle1),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            roles_settings: Default::default(),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();

    {
        // Fail to install the "dependent" app because the dependent DNA hash is not set in the manifest
        let err = conductor
            .clone()
            .install_app_bundle(InstallAppPayload {
                agent_key: None,
                source: AppBundleSource::Bundle(bundle2(false).await),
                installed_app_id: Some("app_2".into()),
                network_seed: None,
                roles_settings: Default::default(),
                ignore_genesis_failure: false,
                allow_throwaway_random_agent_key: true,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ConductorError::AppBundleError(AppBundleError::AppManifestError(_))
        ));
    }
    {
        // Fail to install the dependent app because the existing CellId is not specified
        let err = conductor
            .clone()
            .install_app_bundle(InstallAppPayload {
                agent_key: None,
                source: AppBundleSource::Bundle(bundle2(true).await),
                installed_app_id: Some("app_2".into()),
                network_seed: None,
                roles_settings: Default::default(),
                ignore_genesis_failure: false,
                allow_throwaway_random_agent_key: true,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ConductorError::AppBundleError(AppBundleError::CellResolutionFailure(_, _))
        ));
    }

    // Get the existing cell id through the normal means
    let appmap = conductor
        .cells_by_dna_lineage(dna1.dna_hash())
        .await
        .unwrap();
    assert_eq!(appmap.len(), 1);
    let (app_name, cells) = appmap.first().unwrap();
    assert_eq!(app_name, "app_1");
    assert_eq!(cells.len(), 1);
    let cell_id = cells.first().unwrap().clone();

    let role_settings = ("extant".into(), RoleSettings::UseExisting(cell_id));

    let app_2 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: None,
            source: AppBundleSource::Bundle(bundle2(true).await),
            installed_app_id: Some("app_2".into()),
            network_seed: None,
            roles_settings: Some(HashMap::from([role_settings])),
            ignore_genesis_failure: false,
            allow_throwaway_random_agent_key: true,
        })
        .await
        .unwrap();

    let cell_id_1 = app_1.all_cells().next().unwrap().clone();
    let cell_id_2 = app_2.all_cells().next().unwrap().clone();
    let zome2 = SweetZome::new(cell_id_2.clone(), "whoami".into());

    conductor.enable_app("app_1".into()).await.unwrap();
    conductor.enable_app("app_2".into()).await.unwrap();
    {
        // - Call the existing dependency cell via the dependent cell, which fails
        // because the proper capability has not been granted
        let r: Result<AgentInfo, _> = conductor
            .call_fallible(&zome2, "who_are_they_role", "extant".to_string())
            .await;
        assert!(r.is_err());
    }

    {
        // - Grant the capability
        let secret = CapSecret::from([1; 64]);
        conductor
            .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
                cell_id: cell_id_1.clone(),
                cap_grant: ZomeCallCapGrant {
                    tag: "tag".into(),
                    // access: CapAccess::Unrestricted,
                    access: CapAccess::Transferable { secret },
                    functions: GrantedFunctions::All,
                },
            })
            .await
            .unwrap();

        // - Call the existing dependency cell via the dependent cell
        let r: AgentInfo = conductor
            .call_from_fallible(
                cell_id_2.agent_pubkey(),
                None,
                &zome2,
                "who_are_they_role_secret",
                ("extant".to_string(), Some(secret)),
            )
            .await
            .unwrap();
        assert_eq!(r.agent_initial_pubkey, *cell_id_1.agent_pubkey());
    }

    // Ideally, we shouldn't be able to disable app_1 because it's depended on by enabled app_2.
    // For now, we are just emitting warnings about this.
    conductor
        .disable_app("app_1".into(), DisabledAppReason::User)
        .await
        .unwrap();
    conductor
        .disable_app("app_2".into(), DisabledAppReason::User)
        .await
        .unwrap();
    conductor
        .disable_app("app_1".into(), DisabledAppReason::User)
        .await
        .unwrap();

    // Can't uninstall app because of dependents
    let err = conductor
        .clone()
        .uninstall_app(&"app_1".to_string(), false)
        .await
        .unwrap_err();
    assert_matches!(
        err,
        ConductorError::AppHasDependents(a, b) if a == *"app_1" && b == vec!["app_2".to_string()]
    );

    // Can still uninstall app with force
    conductor
        .clone()
        .uninstall_app(&"app_1".to_string(), true)
        .await
        .unwrap();
}
