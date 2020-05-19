use super::{
    config::{AdminInterfaceConfig, ConductorConfig, DpkiConfig, InterfaceDriver},
    error::ConductorError,
    state::InterfaceConfig,
    ConductorBuilder, ConductorHandle,
};
use holo_hash::*;
use holochain_types::{
    cell::CellId,
    dna::{DnaError, DnaFile},
};
use legacy::InterfaceDriver as LegacyInterfaceDriver;
use std::fs;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tracing::*;

#[derive(Debug, Error)]
pub enum CompatConfigError {
    #[error("Legacy config contains a broken reference: {0}")]
    BrokenReference(String),

    #[error(transparent)]
    ConductorError(#[from] ConductorError),

    #[error(transparent)]
    DnaError(#[from] DnaError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub async fn load_conductor_from_legacy_config(
    legacy: legacy::Config,
    builder: ConductorBuilder,
    agent_pubkey: AgentPubKey,
) -> Result<ConductorHandle, CompatConfigError> {
    use CompatConfigError::*;

    let config = config_from_legacy(&legacy);

    let conductor: ConductorHandle = builder.config(config).build().await?;

    fn dna_key(path: &Path, uuid: &Option<String>) -> String {
        format!("{:?} ; {:?}", path, uuid)
    }

    let mut dna_hashes: HashMap<String, DnaHash> = HashMap::new();
    for dna_config in legacy.dnas.clone() {
        let mut buffer = Vec::new();
        let path: PathBuf = dna_config.file.clone().into();
        fs::File::open(&path)?.read_to_end(&mut buffer)?;
        let mut dna_file = DnaFile::from_file_content(&mut buffer).await?;
        if let Some(uuid) = dna_config.uuid.clone() {
            dna_file = dna_file.with_uuid(uuid).await?;
        }
        dna_hashes.insert(
            dna_key(&path, &dna_config.uuid),
            dna_file.dna_hash().clone(),
        );
        conductor.install_dna(dna_file).await?;
    }

    let mut cell_ids = vec![];
    for i in legacy.instances.clone() {
        // NB: disregarding agent config for now, using a hard-coded pre-made one
        let dna_config = legacy
            .dna_by_id(&i.dna)
            .ok_or_else(|| BrokenReference(format!("No DNA for id: {}", i.dna)))?;
        // make sure we have installed this DNA
        let dna_hash = dna_hashes
            .get(&dna_key(
                &PathBuf::from(dna_config.file.clone()),
                &dna_config.uuid,
            ))
            .ok_or_else(|| BrokenReference(format!("No DNA for path: {}", dna_config.file)))?
            .clone();
        let cell_id = CellId::new(dna_hash, agent_pubkey.clone());
        cell_ids.push((cell_id, None));
    }

    let app_interfaces = extract_app_interfaces(legacy.interfaces);

    conductor.add_cell_ids_to_db(cell_ids).await?;
    conductor.setup_cells(conductor.clone()).await?;

    for i in app_interfaces {
        let InterfaceConfig {
            driver: InterfaceDriver::Websocket { port },
            cells: _,
        } = i;
        conductor
            .add_app_interface_via_handle(port, conductor.clone())
            .await?;
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

fn convert_interface_driver(legacy: LegacyInterfaceDriver) -> Option<InterfaceDriver> {
    match legacy {
        LegacyInterfaceDriver::Websocket { port } => Some(InterfaceDriver::Websocket { port }),
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

fn extract_app_interfaces(legacy_interfaces: Vec<legacy::InterfaceConfig>) -> Vec<InterfaceConfig> {
    legacy_interfaces
        .into_iter()
        .filter(|c| !c.admin)
        .filter_map(|c: legacy::InterfaceConfig| {
            convert_interface_driver(c.driver).map(|driver| InterfaceConfig {
                driver,
                // FIXME: cells not hooked up for now since we don't use signals yet
                cells: Vec::new(),
            })
        })
        .collect()
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::conductor::{
        handle::mock::MockConductorHandle, paths::EnvironmentRootPath, Conductor,
    };
    use holochain_types::test_utils::{fake_agent_pubkey_1, fake_dna_file};
    use legacy as lc;
    use matches::assert_matches;
    use mockall::predicate;
    use std::path::PathBuf;
    use tempdir::TempDir;

    fn legacy_fixtures() -> (lc::Config, EnvironmentRootPath, TempDir) {
        let dir = TempDir::new("").unwrap();
        let dnas = vec![
            lc::DnaConfig {
                id: "a1".to_string(),
                file: dir.path().join("a.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: None,
            },
            lc::DnaConfig {
                id: "a2".to_string(),
                file: dir.path().join("a.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: Some("significant-uuid".to_string()),
            },
            lc::DnaConfig {
                id: "b".to_string(),
                file: dir.path().join("b.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: None,
            },
        ];
        let instances = vec![
            lc::InstanceConfig {
                agent: "".to_string(),
                dna: "a1".to_string(),
                id: "".to_string(),
                storage: lc::StorageConfiguration::Memory,
            },
            lc::InstanceConfig {
                agent: "".to_string(),
                dna: "a2".to_string(),
                id: "".to_string(),
                storage: lc::StorageConfiguration::Memory,
            },
        ];
        let interfaces = vec![
            lc::InterfaceConfig {
                admin: false,
                choose_free_port: None,
                driver: lc::InterfaceDriver::Websocket { port: 1111 },
                id: "".to_string(),
                instances: vec![],
            },
            lc::InterfaceConfig {
                admin: true,
                choose_free_port: None,
                driver: lc::InterfaceDriver::Websocket { port: 2222 },
                id: "".to_string(),
                instances: vec![],
            },
        ];

        let dpki = lc::DpkiConfig {
            instance_id: "foo".into(),
            init_params: "bar".into(),
        };

        let persistence_dir = PathBuf::from("persistence_dir");

        let legacy_config = lc::Config {
            dnas: dnas.clone(),
            instances: instances.clone(),
            interfaces: interfaces.clone(),
            dpki: Some(dpki.clone()),
            persistence_dir: persistence_dir.clone(),
            ..Default::default()
        };

        (legacy_config, persistence_dir.into(), dir)
    }

    #[tokio::test]
    async fn test_config_from_legacy() {
        let (legacy_config, persistence_dir, _) = legacy_fixtures();
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
        let (legacy_config, _, dir) = legacy_fixtures();
        let dna1 = fake_dna_file("A8d8nifNnj");
        let dna2 = fake_dna_file("90jmi9oINoiO");
        let agent_pubkey = fake_agent_pubkey_1();

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

        let expected_cell_ids = vec![
            (
                CellId::new(dna1.dna_hash().clone(), agent_pubkey.clone()),
                None,
            ),
            (
                CellId::new(dna1a.dna_hash().clone(), agent_pubkey.clone()),
                None,
            ),
        ];

        let mut handle = MockConductorHandle::new();
        handle
            .expect_sync_install_dna()
            .with(predicate::eq(dna1))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_sync_install_dna()
            .with(predicate::eq(dna1a))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_sync_install_dna()
            .with(predicate::eq(dna2))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_sync_add_cell_ids_to_db()
            .with(predicate::eq(expected_cell_ids))
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_sync_setup_cells()
            .times(1)
            .returning(|_| Ok(()));
        handle
            .expect_sync_add_app_interface_via_handle()
            .with(predicate::eq(1111), predicate::always())
            .times(1)
            .returning(|port, _| Ok(port));

        let builder = Conductor::builder().with_mock_handle(handle).await;
        let _ = load_conductor_from_legacy_config(legacy_config, builder, agent_pubkey)
            .await
            .unwrap();
    }
}
