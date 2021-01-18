use std::path::Path;
use std::path::PathBuf;

use crate::expect_variant;
use crate::run::run_async;
use crate::CmdRunner;
use anyhow::anyhow;
use anyhow::ensure;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_types::prelude::InstallAppDnaPayload;
use holochain_types::prelude::InstallAppPayload;
use holochain_types::prelude::InstalledAppId;
use walkdir::WalkDir;

pub(crate) async fn attach_app_port(app_port: u16, admin_port: u16) -> anyhow::Result<()> {
    let mut c = CmdRunner::new(admin_port).await;
    let r = AdminRequest::AttachAppInterface {
        port: Some(app_port),
    };
    c.command(r).await?;
    Ok(())
}

pub async fn install_app(
    holochain_path: &Path,
    path: PathBuf,
    dnas: Vec<PathBuf>,
    app_id: InstalledAppId,
) -> anyhow::Result<()> {
    let conductor = run_async(holochain_path, path, None).await?;
    let mut cmd = CmdRunner::new(conductor.0).await;
    let agent_key = cmd.command(AdminRequest::GenerateAgentPubKey).await?;
    let agent_key = expect_variant!(agent_key => AdminResponse::AgentPubKeyGenerated, "Failed to generate agent");

    // Turn dnas into payloads
    let dnas = dnas
        .into_iter()
        .inspect(|path| {
            if !path.is_file() {
                panic!(format!("Dna path {} must be a file", path.display()))
            }
        })
        .enumerate()
        .map(|(i, path)| InstallAppDnaPayload::path_only(path, format!("{}-{}", app_id, i)))
        .collect::<Vec<_>>();

    let app = InstallAppPayload {
        installed_app_id: app_id,
        agent_key,
        dnas,
    };

    let r = AdminRequest::InstallApp(app.into());
    let installed_app = cmd.command(r).await?;
    let installed_app =
        expect_variant!(installed_app => AdminResponse::AppInstalled, "Failed to install app");
    let r = AdminRequest::ActivateApp {
        installed_app_id: installed_app.installed_app_id,
    };
    cmd.command(r).await?;
    let r = AdminRequest::RequestAgentInfo { cell_id: None };
    let agent_info = cmd.command(r).await?;
    msg!("Agent info {:?}", agent_info);
    Ok(())
}

pub fn parse_dnas(mut dnas: Vec<PathBuf>) -> anyhow::Result<Vec<PathBuf>> {
    if dnas.is_empty() {
        dnas.push(std::env::current_dir()?);
    }
    for dna in dnas.iter_mut() {
        if dna.is_dir() {
            let file_path = search_for_dna(&dna)?;
            *dna = file_path;
        }
        ensure!(
            dna.file_name()
                .map(|f| f.to_string_lossy().ends_with(".dna.gz"))
                .unwrap_or(false),
            "File {} is not a valid dna file name: (e.g. my-dna.dna.gz)",
            dna.display()
        );
    }
    Ok(dnas)
}

fn search_for_dna(dna: &Path) -> anyhow::Result<PathBuf> {
    let dir = WalkDir::new(dna)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|d| d.file_type().is_file())
        .find(|f| f.file_name().to_string_lossy().ends_with(".dna.gz"))
        .map(|f| f.into_path());
    dir.ok_or_else(|| {
        anyhow!(
            "Could not find a dna (e.g. my-dna.dna.gz) in directory {}",
            dna.display()
        )
    })
}
