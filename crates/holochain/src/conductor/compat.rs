use super::{
    config::{AdminInterfaceConfig, ConductorConfig, DpkiConfig, InterfaceDriver},
    state::InterfaceConfig,
    Conductor, ConductorHandle,
};
use holo_hash::*;
use holochain_2020_legacy::config::{
    Configuration as LegacyConfig, DpkiConfiguration as LegacyDpkiConfig,
    InterfaceConfiguration as LegacyInterfaceConfig, InterfaceDriver as LegacyInterfaceDriver,
};
use holochain_types::{cell::CellId, dna::DnaFile};
use std::fs;
use std::{collections::HashMap, io::Read};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompatConfigError {}

pub async fn load_conductor_from_legacy_config(
    legacy: LegacyConfig,
) -> Result<ConductorHandle, CompatConfigError> {
    use CompatConfigError::*;

    let (admin_interfaces, app_interfaces) = convert_interfaces(legacy.interfaces);

    let config = ConductorConfig {
        environment_path: legacy.persistence_dir.into(),
        dpki: legacy.dpki.map(convert_dpki),
        admin_interfaces: Some(admin_interfaces),
        ..Default::default()
    };

    // We ignore the specified agent config for now, and use a pregenerated test AgentPubKey
    let agent: AgentPubKey = unimplemented!();
    let conductor: ConductorHandle = Conductor::builder()
        .config(config)
        .with_admin()
        .await
        .expect("TODO");

    let mut dna_hashes = HashMap::new();
    for dna_config in legacy.dnas {
        let buffer = Vec::new();
        let path = dna_config.file;
        fs::File::open(path)
            .expect("TODO")
            .read_to_end(&mut buffer)
            .expect("TODO");
        let mut dna_file = DnaFile::from_file_content(&mut buffer).await.expect("TODO");
        if let Some(uuid) = dna_config.uuid {
            dna_file = dna_file.with_uuid(uuid).await.expect("TODO");
        }
        dna_hashes.insert(path, dna_file.dna_hash().clone());
        conductor.install_dna(dna_file).await.expect("TODO");
    }

    let cell_ids: Vec<CellId> = legacy
        .instances
        .into_iter()
        .map(|i| {
            // NB: disregarding agent config for now, using a hard-coded pre-made one
            let dna_config = legacy
                .dna_by_id(&i.dna)
                // .ok_or_else(|| BrokenReference(format!("No DNA by id: {}", i.dna)))
                .expect("TODO");
            // make sure we have installed this DNA
            let dna_hash = dna_hashes.get(&dna_config.file).expect("TODO").clone();
            Ok(CellId::new(dna_hash, agent.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // TODO: hook up app interfaces

    conductor
        .create_cells(cell_ids, conductor.clone())
        .await
        .expect("TODO");

    Ok(conductor)
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

fn convert_interfaces(
    legacy_interfaces: Vec<LegacyInterfaceConfig>,
) -> (Vec<AdminInterfaceConfig>, Vec<InterfaceConfig>) {
    let (admin, app) = legacy_interfaces
        .into_iter()
        .partition::<Vec<LegacyInterfaceConfig>, _>(|c| c.admin);

    (
        admin
            .into_iter()
            .filter_map(|c: LegacyInterfaceConfig| {
                convert_interface_driver(c.driver).map(|driver| AdminInterfaceConfig { driver })
            })
            .collect(),
        app.into_iter()
            .filter_map(|c: LegacyInterfaceConfig| {
                convert_interface_driver(c.driver).map(|driver| InterfaceConfig {
                    driver,
                    // FIXME: cells not hooked up for now since we don't use it
                    cells: Vec::new(),
                })
            })
            .collect(),
    )
}
