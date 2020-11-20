use super::{
    config::{AdminInterfaceConfig, ConductorConfig, DpkiConfig, InterfaceDriver},
    error::ConductorError,
    state::AppInterfaceConfig,
    ConductorBuilder, ConductorHandle,
};
use holo_hash::*;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_keystore::{test_keystore::spawn_test_keystore, KeystoreError};
use holochain_types::{
    app::InstalledCell,
    cell::CellId,
    dna::{DnaError, DnaFile},
};
use std::fs;
use std::{collections::HashMap, io::Read, path::Path};
use thiserror::Error;
use tracing::*;

#[derive(Debug, Error)]
pub enum CompatConfigError {
    #[error("Legacy config contains a broken reference: {0}")]
    BrokenReference(String),

    #[error(transparent)]
    ConductorError(#[from] Box<ConductorError>),

    #[error(transparent)]
    DnaError(#[from] DnaError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    KeystoreError(#[from] KeystoreError),
}

pub async fn load_conductor_from_legacy_config(
    legacy: legacy::Config,
    builder: ConductorBuilder,
) -> Result<ConductorHandle, CompatConfigError> {
    let config = config_from_legacy(&legacy);
    let keystore = spawn_test_keystore().await?;

    let conductor: ConductorHandle = builder
        .config(config)
        .with_keystore(keystore.clone())
        .build()
        .await
        .map_err(Box::new)?;

    fn dna_key(path: &Path, uuid: &Option<String>) -> String {
        format!("{:?} ; {:?}", path, uuid)
    }

    let mut dna_hashes: HashMap<String, DnaHash> = HashMap::new();
    for dna_config in &legacy.dnas {
        let mut buffer = Vec::new();
        let path = Path::new(&dna_config.file);
        fs::File::open(&path)?.read_to_end(&mut buffer)?;
        let mut dna_file = DnaFile::from_file_content(&buffer).await?;
        if let Some(uuid) = dna_config.uuid.clone() {
            dna_file = dna_file.with_uuid(uuid).await?;
        }
        if let Some(properties) = dna_config.properties.clone() {
            dna_file = dna_file.with_properties(properties).await?;
        }
        dna_hashes.insert(
            dna_key(&path, &dna_config.uuid),
            dna_file.dna_hash().clone(),
        );
        conductor.install_dna(dna_file).await.map_err(Box::new)?;
    }
    let mut app_install_payload = Vec::new();

    let mut agent_list: HashMap<String, AgentPubKey> = HashMap::new();
    for i in &legacy.instances {
        let dna_config = legacy.dna_by_id(&i.dna).ok_or_else(|| {
            CompatConfigError::BrokenReference(format!("No DNA for id: {}", i.dna))
        })?;

        // make sure we have installed this DNA
        let dna_hash = dna_hashes
            .get(&dna_key(Path::new(&dna_config.file), &dna_config.uuid))
            .ok_or_else(|| {
                CompatConfigError::BrokenReference(format!("No DNA for path: {}", dna_config.file))
            })?
            .clone();

        let agent_name = i.agent.clone();
        // make sure we create new pubkey for new agents.
        let agent_pubkey = match agent_list.get(&agent_name) {
            Some(pubkey) => pubkey.clone(),
            _ => {
                let pubkey = keystore.generate_sign_keypair_from_pure_entropy().await?;
                agent_list.insert(agent_name, pubkey.clone());
                pubkey
            }
        };
        let cell_id = CellId::new(dna_hash, agent_pubkey.clone());
        let cell_handle = i.id.clone();
        app_install_payload.push((InstalledCell::new(cell_id, cell_handle), None));
    }

    let app_interfaces = extract_app_interfaces(legacy.interfaces);

    // There is only one app and it wont be referenced
    // externally so we can use LEGACY as the id
    let installed_app_id = "LEGACY".to_string();
    conductor
        .clone()
        .install_app(installed_app_id.clone(), app_install_payload)
        .await
        .map_err(Box::new)?;
    conductor
        .activate_app(installed_app_id.clone())
        .await
        .map_err(Box::new)?;
    let errors = conductor.clone().setup_cells().await.map_err(Box::new)?;

    // If there are any errors return the first one
    if let Some(error) = errors.into_iter().next() {
        return Err(Box::new(ConductorError::from(error)).into());
    }

    for i in app_interfaces {
        let AppInterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
            signal_subscriptions: _,
        } = i;
        conductor
            .clone()
            .add_app_interface(port)
            .await
            .map_err(Box::new)?;
    }

    Ok(conductor)
}

fn config_from_legacy(legacy: &legacy::Config) -> ConductorConfig {
    ConductorConfig {
        environment_path: legacy.persistence_dir.clone().into(),
        dpki: legacy.dpki.clone().map(convert_dpki),
        admin_interfaces: Some(extract_admin_interfaces(legacy.interfaces.clone())),
        ..Default::default()
    }
}

fn convert_dpki(legacy: legacy::DpkiConfig) -> DpkiConfig {
    DpkiConfig {
        instance_id: legacy.instance_id,
        init_params: legacy.init_params,
    }
}

fn convert_interface_driver(legacy: legacy::InterfaceDriver) -> Option<InterfaceDriver> {
    match legacy {
        legacy::InterfaceDriver::Websocket { port } => Some(InterfaceDriver::Websocket { port }),
        _ => None,
    }
}

fn extract_admin_interfaces(
    legacy_interfaces: Vec<legacy::InterfaceConfig>,
) -> Vec<AdminInterfaceConfig> {
    legacy_interfaces
        .into_iter()
        .filter(|c| c.admin)
        .filter_map(|c: legacy::InterfaceConfig| {
            convert_interface_driver(c.driver).map(|driver| AdminInterfaceConfig { driver })
        })
        .collect()
}

fn extract_app_interfaces(
    legacy_interfaces: Vec<legacy::InterfaceConfig>,
) -> Vec<AppInterfaceConfig> {
    legacy_interfaces
        .into_iter()
        .filter(|c| !c.admin)
        .filter_map(|c: legacy::InterfaceConfig| {
            convert_interface_driver(c.driver).map(|driver| AppInterfaceConfig {
                driver,
                signal_subscriptions: HashMap::new(),
            })
        })
        .collect()
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::conductor::{handle::MockConductorHandleT, paths::EnvironmentRootPath, Conductor};
    use holochain_types::{app::MembraneProof, test_utils::fake_dna_zomes};
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;
    use mockall::predicate;
    use std::path::PathBuf;
    use tempdir::TempDir;

    fn legacy_parts() -> (
        Vec<legacy::DnaConfig>,
        Vec<legacy::InstanceConfig>,
        Vec<legacy::InterfaceConfig>,
        legacy::DpkiConfig,
        TempDir,
    ) {
        let dir = TempDir::new("").unwrap();
        let dnas = vec![
            legacy::DnaConfig {
                id: "a1".to_string(),
                file: dir.path().join("a.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: None,
                properties: None,
            },
            legacy::DnaConfig {
                id: "a2".to_string(),
                file: dir.path().join("a.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: Some("significant-uuid".to_string()),
                properties: None,
            },
            legacy::DnaConfig {
                id: "b".to_string(),
                file: dir.path().join("b.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: None,
                properties: None,
            },
        ];
        let instances = vec![
            legacy::InstanceConfig {
                agent: "ag1".to_string(),
                dna: "a1".to_string(),
                id: "i1".to_string(),
                storage: legacy::StorageConfiguration::Memory,
            },
            legacy::InstanceConfig {
                agent: "ag2".to_string(),
                dna: "a2".to_string(),
                id: "i2".to_string(),
                storage: legacy::StorageConfiguration::Memory,
            },
        ];
        let interfaces = vec![
            legacy::InterfaceConfig {
                admin: false,
                choose_free_port: None,
                driver: legacy::InterfaceDriver::Websocket { port: 1111 },
                id: "".to_string(),
                instances: vec![],
            },
            legacy::InterfaceConfig {
                admin: true,
                choose_free_port: None,
                driver: legacy::InterfaceDriver::Websocket { port: 2222 },
                id: "".to_string(),
                instances: vec![],
            },
        ];

        let dpki = legacy::DpkiConfig {
            instance_id: "foo".into(),
            init_params: "bar".into(),
        };

        (dnas, instances, interfaces, dpki, dir)
    }

    fn legacy_fixtures_1() -> (legacy::Config, EnvironmentRootPath, TempDir) {
        let (dnas, instances, interfaces, dpki, dir) = legacy_parts();

        let persistence_dir = PathBuf::from("persistence_dir");

        let legacy_config = legacy::Config {
            dnas,
            instances,
            interfaces,
            dpki: Some(dpki),
            persistence_dir: persistence_dir.clone(),
            ..Default::default()
        };

        (legacy_config, persistence_dir.into(), dir)
    }

    fn legacy_fixtures_2() -> (legacy::Config, EnvironmentRootPath, TempDir) {
        let (dnas, instances, interfaces, dpki, dir) = legacy_parts();
        let (mut i1, mut i2) = (instances[0].clone(), instances[1].clone());

        i1.dna = "a1".to_string();
        i2.dna = "a1".to_string();

        let persistence_dir: PathBuf = dir.path().clone().into();

        let legacy_config = legacy::Config {
            dnas: vec![dnas[0].clone()],
            instances: vec![i1, i2],
            interfaces,
            dpki: Some(dpki),
            persistence_dir: persistence_dir.clone(),
            ..Default::default()
        };

        (legacy_config, persistence_dir.into(), dir)
    }

    #[tokio::test]
    async fn test_config_from_legacy() {
        let (legacy_config, persistence_dir, _) = legacy_fixtures_1();
        let config = config_from_legacy(&legacy_config);
        assert_eq!(config.environment_path, persistence_dir);
        assert_matches!(
            config.admin_interfaces.unwrap()[0],
            AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 2222 },
            }
        );
        assert!(config.dpki.is_some());
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_build_conductor_from_legacy() {
        let (legacy_config, _, dir) = legacy_fixtures_1();
        let dna1 = fake_dna_zomes(
            "A8d8nifNnj",
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );
        let dna2 = fake_dna_zomes(
            "90jmi9oINoiO",
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        tokio::fs::write(
            dir.path().join("a.dna.gz"),
            dna1.to_file_content().await.unwrap(),
        )
        .await
        .unwrap();

        tokio::fs::write(
            dir.path().join("b.dna.gz"),
            dna2.to_file_content().await.unwrap(),
        )
        .await
        .unwrap();

        let dna1a = dna1
            .clone()
            .with_uuid("significant-uuid".into())
            .await
            .unwrap();

        let mut handle = MockConductorHandleT::new();
        handle
            .expect_install_dna()
            .with(predicate::eq(dna1.clone()))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_install_dna()
            .with(predicate::eq(dna1a.clone()))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_install_dna()
            .with(predicate::eq(dna2.clone()))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_install_app()
            .with(
                predicate::eq("LEGACY".to_string()),
                predicate::function(move |data: &Vec<(InstalledCell, Option<MembraneProof>)>| {
                    data[0].0.as_id().dna_hash() == dna1.clone().dna_hash()
                        && data[0].0.as_nick() == "i1"
                        && data[1].0.as_id().dna_hash() == dna1a.clone().dna_hash()
                        && data[1].0.as_nick() == "i2"
                }),
            )
            .times(1)
            .returning(|_, _| Ok(()));
        handle
            .expect_activate_app()
            .with(predicate::eq("LEGACY".to_string()))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_setup_cells()
            .times(1)
            .returning(|| Ok(vec![]));
        handle
            .expect_add_app_interface()
            .with(predicate::eq(1111))
            .times(1)
            .returning(|port| Ok(port));

        let builder = Conductor::builder().with_mock_handle(handle);
        let _ = load_conductor_from_legacy_config(legacy_config, builder)
            .await
            .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_build_conductor_from_legacy_regression() {
        let (legacy_config, _, dir) = legacy_fixtures_2();
        let dna1 = fake_dna_zomes(
            "A8d8nifNnj",
            vec![(TestWasm::Foo.into(), TestWasm::Foo.into())],
        );

        tokio::fs::write(
            dir.path().join("a.dna.gz"),
            dna1.to_file_content().await.unwrap(),
        )
        .await
        .unwrap();

        let handle = load_conductor_from_legacy_config(legacy_config, Conductor::builder())
            .await
            .unwrap();

        let shutdown = handle.take_shutdown_handle().await.unwrap();
        handle.shutdown().await;
        shutdown.await.unwrap();
    }
}
