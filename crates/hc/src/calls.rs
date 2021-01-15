use std::convert::TryInto;
use std::path::PathBuf;
use std::todo;

use anyhow::anyhow;
use anyhow::ensure;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_conductor_api::InterfaceDriver;
use holochain_p2p::kitsune_p2p;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::CellId;
use holochain_types::prelude::DnaHash;
use holochain_types::prelude::InstallAppDnaPayload;
use holochain_types::prelude::InstallAppPayload;
use holochain_types::prelude::InstalledCell;
use portpicker::is_free;
use portpicker::pick_unused_port;
use std::convert::TryFrom;

use crate::expect_match;
use crate::run::run_async;
use crate::CmdRunner;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Call {
    #[structopt(short, long, conflicts_with = "running")]
    /// Run a conductor setup at this path then make the call.
    pub path: Option<PathBuf>,
    #[structopt(short, long)]
    /// Call a running conductor on this port.
    pub running: Option<u16>,
    #[structopt(short, long, conflicts_with_all = &["running", "path"])]
    /// Call all the existing conductors.
    /// [unimplemented]
    pub all: bool,
    #[structopt(subcommand)]
    /// The admin request you want to make.
    pub call: AdminRequestCli,
}

#[derive(Debug, StructOpt)]
pub enum AdminRequestCli {
    AddAdminWs(AddAdminWs),
    AddAppWs(AddAppWs),
    InstallApp(InstallApp),
    ListDnas,
    NewAgent,
    ListCells,
    ListActiveApps,
    ActivateApp(ActivateApp),
    DeactivateApp(DeactivateApp),
    DumpState(DumpState),
    ListAgents(ListAgents),
}
#[derive(Debug, StructOpt)]
pub struct AddAdminWs {
    port: Option<u16>,
}

#[derive(Debug, StructOpt)]
pub struct AddAppWs {
    port: Option<u16>,
}

#[derive(Debug, StructOpt)]
pub struct InstallApp {
    #[structopt(short, long, default_value = "test-app")]
    app_id: String,
    #[structopt(short, long, parse(try_from_str = parse_agent_key))]
    /// If not set then a key will be generated.
    /// Agent key is Base64 (same format that is used in logs).
    agent_key: Option<AgentPubKey>,
    #[structopt(required = true, min_values = 1)]
    /// List of dnas to install.
    dnas: Vec<PathBuf>,
}

#[derive(Debug, StructOpt)]
pub struct ActivateApp {
    app_id: String,
}

#[derive(Debug, StructOpt)]
pub struct DeactivateApp {
    app_id: String,
}

#[derive(Debug, StructOpt)]
pub struct DumpState {
    #[structopt(short, long, parse(try_from_str = parse_agent_key))]
    agent_key: AgentPubKey,
    #[structopt(short, long, parse(try_from_str = parse_dna_hash))]
    dna: DnaHash,
}
#[derive(Debug, StructOpt)]
pub struct ListAgents {
    #[structopt(short, long, parse(try_from_str = parse_agent_key), requires = "dna")]
    agent_key: Option<AgentPubKey>,
    #[structopt(short, long, parse(try_from_str = parse_dna_hash), requires = "agent_key")]
    dna: Option<DnaHash>,
}

pub async fn call(req: Call) -> anyhow::Result<()> {
    let Call {
        path,
        running,
        all,
        call,
    } = req;
    if all {
        todo!("Calling all existing is coming soon");
    }
    let (mut cmd, _h) = match (path, running) {
        (None, Some(running)) => (CmdRunner::new(running).await, None),
        (Some(path), None) => {
            let (port, holochain) = run_async(path, None).await?;
            (CmdRunner::new(port).await, Some(holochain))
        }
        (None, None) => todo!("Calling from existing is coming soon"),
        _ => unreachable!("Can't use path to conductor and running at the same time"),
    };
    match call {
        AdminRequestCli::AddAdminWs(args) => {
            let port = add_admin_interface(&mut cmd, args).await?;
            msg!("Added Admin port {}", port);
        }
        AdminRequestCli::AddAppWs(args) => {
            let port = attach_app_interface(&mut cmd, args).await?;
            msg!("Added App port {}", port);
        }
        AdminRequestCli::InstallApp(args) => {
            let app_id = args.app_id.clone();
            let cells = install_app(&mut cmd, args).await?;
            msg!("Installed App: {} with cells {:?}", app_id, cells);
        }
        AdminRequestCli::ListDnas => {
            let dnas = list_dnas(&mut cmd).await?;
            msg!("Dnas: {:?}", dnas);
        }
        AdminRequestCli::NewAgent => {
            let agent = generate_agent_pub_key(&mut cmd).await?;
            msg!("Added agent {}", agent);
        }
        AdminRequestCli::ListCells => {
            let cells = list_cell_ids(&mut cmd).await?;
            msg!("Cell Ids: {:?}", cells);
        }
        AdminRequestCli::ListActiveApps => {
            let apps = list_active_apps(&mut cmd).await?;
            msg!("Active Apps: {:?}", apps);
        }
        AdminRequestCli::ActivateApp(args) => {
            let app_id = args.app_id.clone();
            activate_app(&mut cmd, args).await?;
            msg!("Activated app: {:?}", app_id);
        }
        AdminRequestCli::DeactivateApp(args) => {
            let app_id = args.app_id.clone();
            deactivate_app(&mut cmd, args).await?;
            msg!("Deactivated app: {:?}", app_id);
        }
        AdminRequestCli::DumpState(args) => {
            let state = dump_state(&mut cmd, args).await?;
            msg!("DUMP STATE \n{}", state);
        }
        AdminRequestCli::ListAgents(args) => {
            use std::fmt::Write;
            let agent_infos = request_agent_info(&mut cmd, args).await?;
            for info in agent_infos {
                let mut out = String::new();
                let cell_info = list_cell_ids(&mut cmd).await?;
                let agents = cell_info
                    .iter()
                    .map(|c| c.agent_pubkey().clone())
                    .map(|a| (a.clone(), holochain_p2p::agent_holo_to_kit(a)))
                    .collect::<Vec<_>>();

                let dnas = cell_info
                    .iter()
                    .map(|c| c.dna_hash().clone())
                    .map(|d| (d.clone(), holochain_p2p::space_holo_to_kit(d)))
                    .collect::<Vec<_>>();

                let info: kitsune_p2p::agent_store::AgentInfo = (&info).try_into().unwrap();
                let this_agent = agents.iter().find(|a| *info.as_agent_ref() == a.1).unwrap();
                let this_dna = dnas.iter().find(|d| *info.as_space_ref() == d.1).unwrap();
                writeln!(out, "This Agent {:?} is {:?}", this_agent.0, this_agent.1)?;
                writeln!(out, "This DNA {:?} is {:?}", this_dna.0, this_dna.1)?;

                use chrono::{DateTime, Duration, NaiveDateTime, Utc};
                let duration = Duration::milliseconds(info.signed_at_ms() as i64);
                let s = duration.num_seconds() as i64;
                let n = duration.clone().to_std().unwrap().subsec_nanos();
                let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
                let exp = dt + Duration::milliseconds(info.expires_after_ms() as i64);
                let now = Utc::now();

                writeln!(out, "signed at {}", dt)?;
                writeln!(
                    out,
                    "expires at {} in {}mins",
                    exp,
                    (exp - now).num_minutes()
                )?;
                writeln!(out, "space: {:?}", info.as_space_ref())?;
                writeln!(out, "agent: {:?}", info.as_agent_ref())?;
                writeln!(out, "urls: {:?}", info.as_urls_ref())?;
                msg!("{}\n", out);
            }
        }
    }
    Ok(())
}

pub async fn add_admin_interface(cmd: &mut CmdRunner, args: AddAdminWs) -> anyhow::Result<u16> {
    let port = match args.port {
        Some(port) => {
            ensure!(is_free(port), "port {} is not free", port);
            port
        }
        None => pick_unused_port().expect("No available ports free"),
    };
    let resp = cmd
        .command(AdminRequest::AddAdminInterfaces(vec![
            AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port },
            },
        ]))
        .await?;
    ensure!(
        matches!(resp, AdminResponse::AdminInterfacesAdded),
        "Failed to add admin interface, got: {:?}",
        resp
    );
    Ok(port)
}

pub async fn install_app(
    cmd: &mut CmdRunner,
    args: InstallApp,
) -> anyhow::Result<Vec<InstalledCell>> {
    let InstallApp {
        app_id,
        agent_key,
        dnas,
    } = args;
    let agent_key = match agent_key {
        Some(agent) => agent,
        None => generate_agent_pub_key(cmd).await?,
    };

    for path in &dnas {
        ensure!(path.is_file(), "Dna path {} must be a file", path.display());
    }

    // Turn dnas into payloads
    let dnas = dnas
        .into_iter()
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
        expect_match!(installed_app => AdminResponse::AppInstalled, "Failed to install app");
    activate_app(
        cmd,
        ActivateApp {
            app_id: installed_app.installed_app_id,
        },
    )
    .await?;
    Ok(installed_app.cell_data)
}

pub async fn list_dnas(cmd: &mut CmdRunner) -> anyhow::Result<Vec<DnaHash>> {
    let resp = cmd.command(AdminRequest::ListDnas).await?;
    Ok(expect_match!(resp => AdminResponse::DnasListed, "Failed to list dnas"))
}

pub async fn generate_agent_pub_key(cmd: &mut CmdRunner) -> anyhow::Result<AgentPubKey> {
    let resp = cmd.command(AdminRequest::GenerateAgentPubKey).await?;
    Ok(
        expect_match!(resp => AdminResponse::AgentPubKeyGenerated, "Failed to generate agent pubkey"),
    )
}

pub async fn list_cell_ids(cmd: &mut CmdRunner) -> anyhow::Result<Vec<CellId>> {
    let resp = cmd.command(AdminRequest::ListCellIds).await?;
    Ok(expect_match!(resp => AdminResponse::CellIdsListed, "Failed to list cell ids"))
}

pub async fn list_active_apps(cmd: &mut CmdRunner) -> anyhow::Result<Vec<String>> {
    let resp = cmd.command(AdminRequest::ListActiveApps).await?;
    Ok(expect_match!(resp => AdminResponse::ActiveAppsListed, "Failed to list active apps"))
}

pub async fn activate_app(cmd: &mut CmdRunner, args: ActivateApp) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::ActivateApp {
            installed_app_id: args.app_id,
        })
        .await?;
    ensure!(
        matches!(resp, AdminResponse::AppActivated),
        "Failed to activate app, got: {:?}",
        resp
    );
    Ok(())
}

pub async fn deactivate_app(cmd: &mut CmdRunner, args: DeactivateApp) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::DeactivateApp {
            installed_app_id: args.app_id,
        })
        .await?;
    ensure!(
        matches!(resp, AdminResponse::AppDeactivated),
        "Failed to deactivate app, got: {:?}",
        resp
    );
    Ok(())
}

pub async fn attach_app_interface(cmd: &mut CmdRunner, args: AddAppWs) -> anyhow::Result<u16> {
    if let Some(port) = args.port {
        ensure!(is_free(port), "port {} is not free", port);
    }
    let resp = cmd
        .command(AdminRequest::AttachAppInterface { port: args.port })
        .await?;
    match resp {
        AdminResponse::AppInterfaceAttached { port } => Ok(port),
        _ => Err(anyhow!(
            "Failed to attach app interface {:?}, got: {:?}",
            args.port,
            resp
        )),
    }
}

pub async fn dump_state(cmd: &mut CmdRunner, args: DumpState) -> anyhow::Result<String> {
    let resp = cmd
        .command(AdminRequest::DumpState {
            cell_id: Box::new(args.into()),
        })
        .await?;
    Ok(expect_match!(resp => AdminResponse::StateDumped, "Failed to dump state"))
}

pub async fn add_agent_info(cmd: &mut CmdRunner, args: Vec<AgentInfoSigned>) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::AddAgentInfo { agent_infos: args })
        .await?;
    ensure!(
        matches!(resp, AdminResponse::AgentInfoAdded),
        "Failed to add agent info, got: {:?}",
        resp
    );
    Ok(())
}

pub async fn request_agent_info(
    cmd: &mut CmdRunner,
    args: ListAgents,
) -> anyhow::Result<Vec<AgentInfoSigned>> {
    let resp = cmd
        .command(AdminRequest::RequestAgentInfo {
            cell_id: args.into(),
        })
        .await?;
    Ok(expect_match!(resp => AdminResponse::AgentInfoRequested, "Failed to request agent info"))
}

fn parse_agent_key(arg: &str) -> anyhow::Result<AgentPubKey> {
    AgentPubKey::try_from(arg).map_err(|e| anyhow::anyhow!("{:?}", e))
}

fn parse_dna_hash(arg: &str) -> anyhow::Result<DnaHash> {
    DnaHash::try_from(arg).map_err(|e| anyhow::anyhow!("{:?}", e))
}

impl From<CellId> for DumpState {
    fn from(cell_id: CellId) -> Self {
        let (dna, agent_key) = cell_id.into_dna_and_agent();
        Self { agent_key, dna }
    }
}

impl From<DumpState> for CellId {
    fn from(ds: DumpState) -> Self {
        CellId::new(ds.dna, ds.agent_key)
    }
}

impl From<ListAgents> for Option<CellId> {
    fn from(la: ListAgents) -> Self {
        let ListAgents {
            agent_key: a,
            dna: d,
        } = la;
        d.and_then(|d| a.map(|a| (d, a)))
            .map(|(d, a)| CellId::new(d, a))
    }
}
