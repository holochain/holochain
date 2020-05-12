use super::{
    config::{AdminInterfaceConfig, ConductorConfig, DpkiConfig, InterfaceDriver},
    error::ConductorError,
    state::InterfaceConfig,
    Conductor, ConductorBuilder, ConductorHandle,
};
use holo_hash::*;
use holochain_2020_legacy::config::{
    Configuration as LegacyConfig, DpkiConfiguration as LegacyDpkiConfig,
    InterfaceConfiguration as LegacyInterfaceConfig, InterfaceDriver as LegacyInterfaceDriver,
};
use holochain_types::{
    cell::CellId,
    dna::{DnaError, DnaFile},
    test_utils::fake_agent_pubkey_1,
};
use std::fs;
use std::{collections::HashMap, io::Read, path::PathBuf};
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
    legacy: LegacyConfig,
    builder: ConductorBuilder,
) -> Result<ConductorHandle, CompatConfigError> {
    use CompatConfigError::*;

    let config = config_from_legacy(&legacy);

    // We ignore the specified agent config for now, and use a pregenerated test AgentPubKey
    // FIXME: use a real agent!
    warn!("Using a constant fake agent. FIXME: use a proper test agent");
    let agent: AgentPubKey = fake_agent_pubkey_1();

    let conductor: ConductorHandle = builder.config(config).with_admin().await?;

    let mut dna_hashes: HashMap<PathBuf, DnaHash> = HashMap::new();
    for dna_config in legacy.dnas.clone() {
        let mut buffer = Vec::new();
        let path: PathBuf = dna_config.file.clone().into();
        fs::File::open(&path)?.read_to_end(&mut buffer)?;
        let mut dna_file = DnaFile::from_file_content(&mut buffer).await?;
        if let Some(uuid) = dna_config.uuid.clone() {
            dna_file = dna_file.with_uuid(uuid).await?;
        }
        dna_hashes.insert(path, dna_file.dna_hash().clone());
        conductor.install_dna(dna_file).await?;
    }

    let mut cell_ids: Vec<CellId> = vec![];
    for i in legacy.instances.clone() {
        // NB: disregarding agent config for now, using a hard-coded pre-made one
        let dna_config = legacy
            .dna_by_id(&i.dna)
            .ok_or_else(|| BrokenReference(format!("No DNA for id: {}", i.dna)))?;
        // make sure we have installed this DNA
        let dna_hash = dna_hashes
            .get(&PathBuf::from(dna_config.file.clone()))
            .ok_or_else(|| BrokenReference(format!("No DNA for path: {}", dna_config.file)))?
            .clone();
        cell_ids.push(CellId::new(dna_hash, agent.clone()));
    }

    // TODO: hook up app interfaces
    let _app_interfaces = extract_app_interfaces(legacy.interfaces);

    conductor.create_cells(cell_ids, conductor.clone()).await?;

    Ok(conductor)
}

fn config_from_legacy(legacy: &LegacyConfig) -> ConductorConfig {
    ConductorConfig {
        environment_path: legacy.persistence_dir.clone().into(),
        dpki: legacy.dpki.clone().map(convert_dpki),
        admin_interfaces: Some(extract_admin_interfaces(legacy.interfaces.clone())),
        ..Default::default()
    }
}

fn convert_dpki(legacy: LegacyDpkiConfig) -> DpkiConfig {
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
    legacy_interfaces: Vec<LegacyInterfaceConfig>,
) -> Vec<AdminInterfaceConfig> {
    legacy_interfaces
        .into_iter()
        .filter(|c| c.admin)
        .filter_map(|c: LegacyInterfaceConfig| {
            convert_interface_driver(c.driver).map(|driver| AdminInterfaceConfig { driver })
        })
        .collect()
}

fn extract_app_interfaces(legacy_interfaces: Vec<LegacyInterfaceConfig>) -> Vec<InterfaceConfig> {
    legacy_interfaces
        .into_iter()
        .filter(|c| !c.admin)
        .filter_map(|c: LegacyInterfaceConfig| {
            convert_interface_driver(c.driver).map(|driver| InterfaceConfig {
                driver,
                // FIXME: cells not hooked up for now since we don't use it
                cells: Vec::new(),
            })
        })
        .collect()
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::conductor::{handle::mock::MockConductorHandle, paths::EnvironmentRootPath};
    use holochain_2020_legacy::config as lc;
    use holochain_types::test_utils::fake_dna_file;
    use matches::assert_matches;
    use mockall::predicate;
    use std::path::PathBuf;
    use tempdir::TempDir;

    fn legacy_fixtures() -> (
        lc::Configuration,
        Vec<lc::DnaConfiguration>,
        Vec<lc::InstanceConfiguration>,
        Vec<lc::InterfaceConfiguration>,
        EnvironmentRootPath,
        TempDir,
    ) {
        let dir = TempDir::new("").unwrap();
        let dnas = vec![
            lc::DnaConfiguration {
                id: "a1".to_string(),
                file: dir.path().join("a.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: None,
            },
            lc::DnaConfiguration {
                id: "a2".to_string(),
                file: dir.path().join("a.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: Some("a".to_string()),
            },
            lc::DnaConfiguration {
                id: "b".to_string(),
                file: dir.path().join("b.dna.gz").to_string_lossy().into(),
                hash: "".to_string(),
                uuid: None,
            },
        ];
        let instances = vec![
            lc::InstanceConfiguration {
                agent: "".to_string(),
                dna: "a1".to_string(),
                id: "".to_string(),
                storage: lc::StorageConfiguration::Memory,
            },
            lc::InstanceConfiguration {
                agent: "".to_string(),
                dna: "a2".to_string(),
                id: "".to_string(),
                storage: lc::StorageConfiguration::Memory,
            },
        ];
        let interfaces = vec![
            lc::InterfaceConfiguration {
                admin: false,
                choose_free_port: None,
                driver: lc::InterfaceDriver::Websocket { port: 1111 },
                id: "".to_string(),
                instances: vec![],
            },
            lc::InterfaceConfiguration {
                admin: true,
                choose_free_port: None,
                driver: lc::InterfaceDriver::Websocket { port: 2222 },
                id: "".to_string(),
                instances: vec![],
            },
        ];

        let dpki = lc::DpkiConfiguration {
            instance_id: "foo".into(),
            init_params: "bar".into(),
        };

        let persistence_dir = PathBuf::from("persistence_dir");

        let legacy_config = lc::Configuration {
            dnas: dnas.clone(),
            instances: instances.clone(),
            interfaces: interfaces.clone(),
            dpki: Some(dpki.clone()),
            persistence_dir: persistence_dir.clone(),
            ..Default::default()
        };

        (
            legacy_config,
            dnas,
            instances,
            interfaces,
            persistence_dir.into(),
            dir,
        )
    }

    #[tokio::test]
    async fn test_config_from_legacy() {
        let (legacy_config, _, _, _, persistence_dir, _) = legacy_fixtures();
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
        let (legacy_config, dnas, instances, interfaces, _, dir) = legacy_fixtures();
        let dna1 = fake_dna_file("a");
        let dna2 = fake_dna_file("b");

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

        let mut handle = MockConductorHandle::new();
        handle
            .expect_sync_install_dna()
            .with(predicate::eq(dna1))
            .times(2)
            .returning(|_| Ok(()));
        handle
            .expect_sync_install_dna()
            .with(predicate::eq(dna2))
            .times(1)
            .returning(|_| Ok(()));

        handle
            .expect_sync_create_cells()
            .times(1)
            .returning(|_ids, _handle| Ok(()));

        todo!("assert that create_cells is called with the proper CellIds");
        todo!("assert app interfaces created");

        let builder = Conductor::builder().with_mock_handle(handle).await;
        let _ = load_conductor_from_legacy_config(legacy_config, builder)
            .await
            .expect("TODO");
    }
}
