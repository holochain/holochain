use std::collections::HashMap;
use std::path::PathBuf;

use ::fixt::prelude::*;
use holochain::sweettest::*;
use holochain_conductor_api::{AppInfoStatus, CellInfo};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

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

    let dna_hash_0 = match app_info_0
        .cell_info
        .into_iter()
        .find(|(role_name, _)| role_name == &role_name_1)
        .unwrap()
        .1[0]
        .clone()
    {
        CellInfo::Provisioned(c) => c.cell_id.dna_hash().clone(),
        _ => panic!("wrong cell type."),
    };

    let app_info_1 = conductor
        .get_app_info(&"app_1".to_string())
        .await
        .unwrap()
        .unwrap();

    let dna_hash_1 = match app_info_1
        .cell_info
        .into_iter()
        .find(|(role_name, _)| role_name == &role_name_1)
        .unwrap()
        .1[0]
        .clone()
    {
        CellInfo::Provisioned(c) => c.cell_id.dna_hash().clone(),
        _ => panic!("wrong cell type."),
    };

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
