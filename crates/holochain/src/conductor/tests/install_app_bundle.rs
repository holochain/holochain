use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::conductor::api::error::ConductorApiError;
use crate::{conductor::error::ConductorError, sweettest::*};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use maplit::btreeset;
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
async fn clone_only_provisioning_creates_no_cell_and_allows_cloning() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    async fn make_payload(agent_key: AgentPubKey, clone_limit: u32) -> InstallAppPayload {
        // The integrity zome in this WASM will fail if the properties are not set. This helps verify that genesis
        // is not being run for the clone-only cell and will only run for the cloned cells.
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![
            TestWasm::GenesisSelfCheckRequiresProperties,
        ])
        .await;
        let path = PathBuf::from(format!("{}", dna.dna_hash()));
        let modifiers = DnaModifiersOpt::none();
        let installed_dna_hash = DnaHash::with_data_sync(dna.dna_def());

        let roles = vec![AppRoleManifest {
            name: "name".into(),
            dna: AppRoleDnaManifest {
                location: Some(DnaLocation::Bundled(path.clone())),
                modifiers: modifiers.clone(),
                installed_hash: Some(installed_dna_hash.into()),
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
            agent_key,
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
        }
    }

    // Fails due to clone limit of 0
    assert_matches!(
        conductor
            .clone()
            .install_app_bundle(make_payload(agent.clone(), 0).await)
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
            .install_app_bundle(make_payload(agent.clone(), 1).await)
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
        assert_eq!(*clone_cell.cell_id.agent_pubkey(), agent);
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
            ConductorApiError::ConductorError(ConductorError::AppError(
                AppError::CloneLimitExceeded(1, _)
            ))
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
    let alice = SweetAgents::one(conductor.keystore()).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let path = PathBuf::from(format!("{}", dna.dna_hash()));
    let modifiers = DnaModifiersOpt::none();
    let installed_dna_hash = DnaHash::with_data_sync(dna.dna_def());
    let cell_id = CellId::new(dna.dna_hash().to_owned(), alice.clone());

    let roles = vec![AppRoleManifest {
        name: "name".into(),
        dna: AppRoleDnaManifest {
            location: Some(DnaLocation::Bundled(path.clone())),
            modifiers: modifiers.clone(),
            installed_hash: Some(installed_dna_hash.into()),
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
            agent_key: alice.clone(),
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    let resources = vec![(path.clone(), DnaBundle::from_dna_file(dna.clone()).unwrap())];
    let bundle = AppBundle::new(manifest.clone().into(), resources, PathBuf::from("."))
        .await
        .unwrap();
    let duplicate_install_with_app_disabled = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            source: AppBundleSource::Bundle(bundle),
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
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
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
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
            agent_key: alice.clone(),
            installed_app_id: Some("app_2".into()),
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
            network_seed: Some("network".into()),
        })
        .await;
    assert!(valid_install_of_second_app.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn can_install_app_a_second_time_using_nothing_but_the_manifest_from_app_info() {
    let conductor = SweetConductor::from_standard_config().await;
    let (alice, bobbo) = SweetAgents::two(conductor.keystore()).await;

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
            agent_key: alice.clone(),
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_1".into()),
            network_seed: Some("final seed".into()),
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
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
            agent_key: bobbo,
            source: AppBundleSource::Bundle(bundle),
            installed_app_id: Some("app_2".into()),
            network_seed: None,
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
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
    let (alice, bob) = SweetAgents::two(conductor.keystore()).await;

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
            agent_key: alice.clone(),
            source: AppBundleSource::Bundle(bundle1),
            installed_app_id: Some("app_1".into()),
            network_seed: None,
            membrane_proofs: Default::default(),
            existing_cells: Default::default(),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    {
        // Fail to install the "dependent" app because the dependent DNA hash is not set in the manifest
        let err = conductor
            .clone()
            .install_app_bundle(InstallAppPayload {
                agent_key: bob.clone(),
                source: AppBundleSource::Bundle(bundle2(false).await),
                installed_app_id: Some("app_2".into()),
                network_seed: None,
                membrane_proofs: Default::default(),
                existing_cells: Default::default(),
                ignore_genesis_failure: false,
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
                agent_key: bob.clone(),
                source: AppBundleSource::Bundle(bundle2(true).await),
                installed_app_id: Some("app_2".into()),
                network_seed: None,
                membrane_proofs: Default::default(),
                existing_cells: Default::default(),
                ignore_genesis_failure: false,
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

    let app_2 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: bob.clone(),
            source: AppBundleSource::Bundle(bundle2(true).await),
            installed_app_id: Some("app_2".into()),
            network_seed: None,
            membrane_proofs: Default::default(),
            existing_cells: maplit::hashmap! {
                "extant".to_string() => cell_id
            },
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    let cell_id_1 = app_1.all_cells().next().unwrap().clone();
    let cell_id_2 = app_2.all_cells().next().unwrap().clone();
    // let zome1 = SweetZome::new(cell_id_1.clone(), "whoami".into());
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
