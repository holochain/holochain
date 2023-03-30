use std::{collections::HashMap, path::PathBuf};

use crate::{conductor::error::ConductorError, sweettest::*};
use fixt::prelude::strum_macros;
use holo_hash::DnaHash;
use holochain_types::prelude::{
    mapvec, AppBundle, AppBundleSource, AppManifestCurrentBuilder, AppRoleDnaManifest,
    AppRoleManifest, CellProvisioning, DnaBundle, DnaFile, DnaLocation, InstallAppPayload,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{CellId, DnaModifiersOpt};
use matches::assert_matches;
use tempfile::{tempdir, TempDir};

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
async fn network_seed_regression() {
    let conductor = SweetConductor::from_standard_config().await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let tmp = tempdir().unwrap();
    let (dna, _, _) = SweetDnaFile::from_test_wasms(
        "".into(),
        vec![TestWasm::Create],
        holochain_serialized_bytes::SerializedBytes::default(),
    )
    .await;

    let dna_path = tmp.as_ref().join(format!("the.dna"));
    DnaBundle::from_dna_file(dna)
        .await
        .unwrap()
        .write_to_file(&dna_path)
        .await
        .unwrap();

    let manifest = {
        let roles = vec![AppRoleManifest {
            name: "rolename".into(),
            dna: AppRoleDnaManifest {
                location: Some(DnaLocation::Path(dna_path)),
                modifiers: DnaModifiersOpt::default(),
                installed_hash: None,
                clone_limit: 0,
            },
            provisioning: None,
        }];

        AppManifestCurrentBuilder::default()
            .name("app".into())
            .description(None)
            .roles(roles)
            .build()
            .unwrap()
    };

    let bundle1 = AppBundle::new(manifest.clone().into(), vec![], PathBuf::from("."))
        .await
        .unwrap();
    let bundle2 = AppBundle::new(manifest.into(), vec![], PathBuf::from("."))
        .await
        .unwrap();

    // if both of these apps can be installed under the same agent, the
    // network seed change was successful -- otherwise there will be a
    // CellAlreadyInstalled error.

    let _app1 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: agent.clone(),
            source: AppBundleSource::Bundle(bundle1),
            installed_app_id: Some("no-seed".into()),
            network_seed: None,
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();

    let _app2 = conductor
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: agent.clone(),
            source: AppBundleSource::Bundle(bundle2),
            installed_app_id: Some("yes-seed".into()),
            network_seed: Some("seed".into()),
            membrane_proofs: HashMap::new(),
        })
        .await
        .unwrap();
}

/// Test all possible combinations of Locations and network seeds:
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "glacial_tests")]
#[ignore = "this is a really useful comprehensive test, but it's so dang slow"]
async fn network_seed_affects_dna_hash_when_app_bundle_is_installed() {
    let conductor = SweetConductor::from_standard_config().await;
    let tmp = tempdir().unwrap();
    let (dna, _, _) = SweetDnaFile::from_test_wasms(
        "".to_string(),
        vec![TestWasm::Create],
        holochain_serialized_bytes::SerializedBytes::default(),
    )
    .await;

    let write_dna = |seed: Seed| {
        let mut dna = dna.clone();
        let path = tmp.as_ref().join(format!("{}.dna", seed.to_string()));
        async move {
            if seed != Seed::None {
                dna = dna.with_network_seed(seed.to_string()).await;
            }
            DnaBundle::from_dna_file(dna.clone())
                .await
                .unwrap()
                .write_to_file(&path)
                .await
                .unwrap();
            dna
        }
    };

    let dnas = futures::future::join_all(vec![write_dna(None), write_dna(A), write_dna(B)]).await;

    let c = TestcaseCommon {
        conductor,
        dnas: dnas.clone(),
        tmp,
        _start: std::time::Instant::now(),
    };

    use Location::*;
    use Seed::*;

    let both = [Bundle, Path];

    let all_locs = both.iter().flat_map(|a| both.iter().map(|b| (*a, *b)));

    // Build up two equality groups. All outcomes in each group should have equal hashes,
    // and each group's hash should be different from the other group's hash.

    // Hashes when using empty network seed
    let mut group_0 = vec![];
    // Hashes when using network seed "A"
    let mut group_a = vec![];
    // There is no need for a group_b since "A" and "B" are essentially interchangeable

    // Populate the groups with all (most) possible combinations of seed values and location specifiers
    for (app_loc, dna_loc) in all_locs {
        group_0.extend([TestCase(None, None, None, app_loc, dna_loc).install(&c)]);
        group_a.extend([
            TestCase(A, None, None, app_loc, dna_loc).install(&c),
            TestCase(None, A, None, app_loc, dna_loc).install(&c),
            TestCase(None, None, A, app_loc, dna_loc).install(&c),
            //
            TestCase(A, A, None, app_loc, dna_loc).install(&c),
            TestCase(A, None, A, app_loc, dna_loc).install(&c),
            TestCase(None, A, A, app_loc, dna_loc).install(&c),
            //
            TestCase(A, B, None, app_loc, dna_loc).install(&c),
            TestCase(A, None, B, app_loc, dna_loc).install(&c),
            TestCase(None, A, B, app_loc, dna_loc).install(&c),
            //
            TestCase(A, B, B, app_loc, dna_loc).install(&c),
        ]);
    }

    let group_0 = futures::future::join_all(group_0).await;
    let group_a = futures::future::join_all(group_a).await;

    let (hash_0, case_0) = &group_0[0];
    let (hash_a, case_a) = &group_a[0];

    dbg!(mapvec(dnas.iter(), |d| d.dna_hash()));
    dbg!(&hash_0, mapvec(group_0.iter(), |(h, c)| (h, c.to_string())));
    dbg!(&hash_a, mapvec(group_a.iter(), |(h, c)| (h, c.to_string())));

    assert_eq!(hash_0, dnas[0].dna_hash());
    assert_eq!(hash_a, dnas[1].dna_hash());
    assert_ne!(hash_0, hash_a);

    for (h, c) in group_0.iter() {
        assert_eq!(
            hash_0,
            h,
            "case mismatch: {}, {}",
            case_0.to_string(),
            c.to_string()
        );
    }
    for (h, c) in group_a.iter() {
        assert_eq!(
            hash_a,
            h,
            "case mismatch: {}, {}",
            case_a.to_string(),
            c.to_string()
        );
    }
}

struct TestcaseCommon {
    conductor: SweetConductor,
    dnas: Vec<DnaFile>,
    tmp: TempDir,
    _start: std::time::Instant,
}

#[derive(Clone, Copy, Debug, strum_macros::ToString)]
enum Location {
    Path,
    Bundle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::ToString)]
enum Seed {
    None,
    A,
    B,
}

#[derive(Debug)]
struct TestCase(Seed, Seed, Seed, Location, Location);

impl ToString for TestCase {
    fn to_string(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.0.to_string(),
            self.1.to_string(),
            self.2.to_string(),
            self.3.to_string(),
            self.4.to_string(),
        )
    }
}

impl TestCase {
    async fn install(self, common: &TestcaseCommon) -> (DnaHash, Self) {
        let case = self;
        let case_str = case.to_string();
        let TestCase(app_seed, role_seed, dna_seed, app_loc, dna_loc) = case;
        let dna = match dna_seed {
            Seed::None => common.dnas[0].clone(),
            Seed::A => common.dnas[1].clone(),
            Seed::B => common.dnas[2].clone(),
        };
        let dna_hash = dna.dna_hash();
        let agent_key = SweetAgents::one(common.conductor.keystore()).await;

        let dna_modifiers = match role_seed {
            Seed::None => DnaModifiersOpt::none(),
            Seed::A => DnaModifiersOpt::none().with_network_seed(Seed::A.to_string()),
            Seed::B => DnaModifiersOpt::none().with_network_seed(Seed::B.to_string()),
        };

        let dna_path = common
            .tmp
            .as_ref()
            .join(format!("{}.dna", dna_seed.to_string()));

        let bundle = match dna_loc {
            Location::Bundle => {
                let hashpath = PathBuf::from(dna_hash.to_string());
                let roles = vec![AppRoleManifest {
                    name: "rolename".into(),
                    dna: AppRoleDnaManifest {
                        location: Some(DnaLocation::Bundled(hashpath.clone())),
                        modifiers: dna_modifiers.clone(),
                        installed_hash: None,
                        clone_limit: 10,
                    },
                    provisioning: Some(CellProvisioning::Create { deferred: false }),
                }];
                let manifest = AppManifestCurrentBuilder::default()
                    .name(case_str.clone())
                    .description(None)
                    .roles(roles)
                    .build()
                    .unwrap();
                let resources = vec![(
                    hashpath,
                    DnaBundle::from_dna_file(dna.clone()).await.unwrap(),
                )];

                AppBundle::new(manifest.into(), resources, PathBuf::from("."))
                    .await
                    .unwrap()
            }
            Location::Path => {
                // use path
                let roles = vec![AppRoleManifest {
                    name: "rolename".into(),
                    dna: AppRoleDnaManifest {
                        location: Some(DnaLocation::Path(dna_path.clone())),
                        modifiers: dna_modifiers.clone(),
                        installed_hash: Some(dna_hash.clone().into()),
                        clone_limit: 0,
                    },
                    provisioning: None,
                }];

                let manifest = AppManifestCurrentBuilder::default()
                    .name(case_str.clone())
                    .description(None)
                    .roles(roles)
                    .build()
                    .unwrap();
                AppBundle::new(manifest.into(), vec![], PathBuf::from("."))
                    .await
                    .unwrap()
            }
        };

        let network_seed = match app_seed {
            Seed::None => None,
            Seed::A => Some(Seed::A.to_string()),
            Seed::B => Some(Seed::B.to_string()),
        };

        let source = match app_loc {
            Location::Path => {
                // Unnecessary duplication, but it's ok.
                let happ_path = common.tmp.as_ref().join(&case_str);
                bundle.write_to_file(&happ_path).await.unwrap();
                AppBundleSource::Path(happ_path)
            }
            Location::Bundle => AppBundleSource::Bundle(bundle),
        };

        let app = common
            .conductor
            .clone()
            .install_app_bundle(InstallAppPayload {
                agent_key,
                source,
                installed_app_id: Some(case_str.clone()),
                network_seed,
                membrane_proofs: HashMap::new(),
            })
            .await
            .unwrap();

        let installed_hash = app.all_cells().next().unwrap().dna_hash().clone();
        (installed_hash, case)
    }
}
