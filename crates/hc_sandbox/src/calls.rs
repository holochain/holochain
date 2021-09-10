//! Helpers for making [`AdminRequest`]s to the admin api.
//!
//! This module is designed for use in a CLI so it is more simplified
//! then calling the [`CmdRunner`] directly.
//! For simple calls like [`AdminRequest::ListDnas`] this is probably easier
//! but if you want more control use [`CmdRunner::command`].
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use holochain_conductor_api::AdminRequest;
use holochain_conductor_api::AdminResponse;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::InterfaceDriver;
use holochain_conductor_api::{AdminInterfaceConfig, InstalledAppInfo};
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_types::prelude::DnaHash;
use holochain_types::prelude::InstallAppDnaPayload;
use holochain_types::prelude::InstallAppPayload;
use holochain_types::prelude::RegisterDnaPayload;
use holochain_types::prelude::YamlProperties;
use holochain_types::prelude::{AgentPubKey, AppBundleSource};
use holochain_types::prelude::{CellId, InstallAppBundlePayload};
use holochain_types::prelude::{DnaSource, Uid};
use std::convert::TryFrom;

use crate::cmds::Existing;
use crate::expect_match;
use crate::ports::get_admin_ports;
use crate::run::run_async;
use crate::CmdRunner;
use structopt::StructOpt;

#[doc(hidden)]
#[derive(Debug, StructOpt)]
pub struct Call {
    #[structopt(short, long, conflicts_with_all = &["existing_paths", "indices"], value_delimiter = ",")]
    /// Ports to running conductor admin interfaces.
    /// If this is empty existing sandboxes will be used.
    /// Cannot be combined with existing sandboxes.
    pub running: Vec<u16>,
    #[structopt(flatten)]
    pub existing: Existing,
    #[structopt(subcommand)]
    /// The admin request you want to make.
    pub call: AdminRequestCli,
}

// Docs have different use for structopt
// so documenting everything doesn't make sense.
#[allow(missing_docs)]
#[derive(Debug, StructOpt, Clone)]
pub enum AdminRequestCli {
    AddAdminWs(AddAdminWs),
    AddAppWs(AddAppWs),
    RegisterDna(RegisterDna),
    InstallApp(InstallApp),
    InstallAppBundle(InstallAppBundle),
    /// Calls AdminRequest::UninstallApp.
    UninstallApp(UninstallApp),
    /// Calls AdminRequest::ListAppInterfaces.
    ListAppWs,
    /// Calls AdminRequest::ListDnas.
    ListDnas,
    /// Calls AdminRequest::GenerateAgentPubKey.
    NewAgent,
    /// Calls AdminRequest::ListCellIds.
    ListCells,
    /// Calls AdminRequest::ListActiveApps.
    ListActiveApps,
    /// Calls AdminRequest::ListApps.
    ListApps(ListApps),
    EnableApp(EnableApp),
    DisableApp(DisableApp),
    DumpState(DumpState),
    /// Calls AdminRequest::AddAgentInfo.
    /// [Unimplemented].
    AddAgents,
    ListAgents(ListAgents),
}
#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::AddAdminInterfaces
/// and adds another admin interface.
pub struct AddAdminWs {
    /// Optional port number.
    /// Defaults to assigned by OS.
    pub port: Option<u16>,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::AttachAppInterface
/// and adds another app interface.
pub struct AddAppWs {
    /// Optional port number.
    /// Defaults to assigned by OS.
    pub port: Option<u16>,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::RegisterDna
/// and registers a Dna. You can only use a path or a hash not both.
pub struct RegisterDna {
    #[structopt(short, long)]
    /// UID to override when installing this Dna
    pub uid: Option<String>,
    #[structopt(short, long)]
    /// Properties to override when installing this Dna
    pub properties: Option<PathBuf>,
    #[structopt(short, long, conflicts_with = "hash", required_unless = "hash")]
    /// Path to a DnaBundle file.
    pub path: Option<PathBuf>,
    #[structopt(short, long, parse(try_from_str = parse_dna_hash), required_unless = "path")]
    /// Hash of an existing dna you want to register.
    pub hash: Option<DnaHash>,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::InstallApp
/// and installs a new app.
///
/// Setting properties and membrane proofs is not
/// yet supported.
/// CellNicks are set to `my-app-0`, `my-app-1` etc.
pub struct InstallApp {
    #[structopt(short, long, default_value = "test-app")]
    /// Sets the InstalledAppId.
    pub app_id: String,
    #[structopt(short = "i", long, parse(try_from_str = parse_agent_key))]
    /// If not set then a key will be generated.
    /// Agent key is Base64 (same format that is used in logs).
    /// e.g. `uhCAk71wNXTv7lstvi4PfUr_JDvxLucF9WzUgWPNIEZIoPGMF4b_o`
    pub agent_key: Option<AgentPubKey>,
    #[structopt(required = true, min_values = 1, parse(try_from_str = parse_dna_hash))]
    /// The dna hashes to use in this app.
    pub dnas: Vec<DnaHash>,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::InstallAppBundle
/// and installs a new app.
///
/// Setting properties and membrane proofs is not
/// yet supported.
/// CellNicks are set to `my-app-0`, `my-app-1` etc.
pub struct InstallAppBundle {
    #[structopt(short, long)]
    /// Sets the InstalledAppId.
    pub app_id: Option<String>,

    #[structopt(short, long, parse(try_from_str = parse_agent_key))]
    /// If not set then a key will be generated.
    /// Agent key is Base64 (same format that is used in logs).
    /// e.g. `uhCAk71wNXTv7lstvi4PfUr_JDvxLucF9WzUgWPNIEZIoPGMF4b_o`
    pub agent_key: Option<AgentPubKey>,

    #[structopt(required = true)]
    /// Location of the *.happ bundle file to install.
    pub path: PathBuf,

    /// Optional UID override for every DNA in this app
    pub uid: Option<Uid>,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::UninstallApp
/// and uninstalls the specified app.
pub struct UninstallApp {
    /// The InstalledAppId to uninstall.
    pub app_id: String,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::EnableApp
/// and activates the installed app.
pub struct EnableApp {
    /// The InstalledAppId to activate.
    pub app_id: String,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::DisableApp
/// and disables the installed app.
pub struct DisableApp {
    /// The InstalledAppId to disable.
    pub app_id: String,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::DumpState
/// and dumps the current cell's state.
/// TODO: Add pretty print.
/// TODO: Default to dumping all cell state.
pub struct DumpState {
    #[structopt(parse(try_from_str = parse_dna_hash))]
    /// The dna hash half of the cell id to dump.
    pub dna: DnaHash,
    #[structopt(parse(try_from_str = parse_agent_key))]
    /// The agent half of the cell id to dump.
    pub agent_key: AgentPubKey,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::RequestAgentInfo
/// and pretty prints the agent info on
/// this conductor.
pub struct ListAgents {
    #[structopt(short, long, parse(try_from_str = parse_agent_key), requires = "dna")]
    /// Optionally request agent info for a particular cell id.
    pub agent_key: Option<AgentPubKey>,
    #[structopt(short, long, parse(try_from_str = parse_dna_hash), requires = "agent_key")]
    /// Optionally request agent info for a particular cell id.
    pub dna: Option<DnaHash>,
}

#[derive(Debug, StructOpt, Clone)]
/// Calls AdminRequest::ListApps
/// and pretty prints the list of apps
/// installed in this conductor.
pub struct ListApps {
    #[structopt(short, long, parse(try_from_str = parse_status_filter))]
    /// Optionally request agent info for a particular cell id.
    pub status: Option<AppStatusFilter>,
}

#[doc(hidden)]
pub async fn call(holochain_path: &Path, req: Call) -> anyhow::Result<()> {
    let Call {
        existing,
        running,
        call,
    } = req;
    let cmds = if running.is_empty() {
        let paths = if existing.is_empty() {
            crate::save::load(std::env::current_dir()?)?
        } else {
            existing.load()?
        };
        let ports = get_admin_ports(paths.clone()).await?;
        let mut cmds = Vec::with_capacity(ports.len());
        for (port, path) in ports.into_iter().zip(paths.into_iter()) {
            match CmdRunner::try_new(port).await {
                Ok(cmd) => cmds.push((cmd, None)),
                Err(e) => {
                    if let holochain_websocket::WebsocketError::Io(e) = &e {
                        if let std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::AddrNotAvailable = e.kind()
                        {
                            let (port, holochain) = run_async(holochain_path, path, None).await?;
                            cmds.push((CmdRunner::new(port).await, Some(holochain)));
                            continue;
                        }
                    }
                    bail!(
                        "Failed to connect to running conductor or start one {:?}",
                        e
                    )
                }
            }
        }
        cmds
    } else {
        let mut cmds = Vec::with_capacity(running.len());
        for port in running {
            cmds.push((CmdRunner::new(port).await, None));
        }
        cmds
    };
    for mut cmd in cmds {
        call_inner(&mut cmd.0, call.clone()).await?;
    }
    Ok(())
}

async fn call_inner(cmd: &mut CmdRunner, call: AdminRequestCli) -> anyhow::Result<()> {
    match call {
        AdminRequestCli::AddAdminWs(args) => {
            let port = add_admin_interface(cmd, args).await?;
            msg!("Added Admin port {}", port);
        }
        AdminRequestCli::AddAppWs(args) => {
            let port = attach_app_interface(cmd, args).await?;
            msg!("Added App port {}", port);
        }
        AdminRequestCli::ListAppWs => {
            let ports = list_app_ws(cmd).await?;
            msg!("Attached App Interfaces {:?}", ports);
        }
        AdminRequestCli::RegisterDna(args) => {
            let dnas = register_dna(cmd, args).await?;
            msg!("Registered Dna: {:?}", dnas);
        }
        AdminRequestCli::InstallApp(args) => {
            let app_id = args.app_id.clone();
            let _ = install_app(cmd, args).await?;
            msg!("Installed App: {}", app_id);
        }
        AdminRequestCli::InstallAppBundle(args) => {
            let app = install_app_bundle(cmd, args).await?;
            msg!("Installed App: {}", app.installed_app_id,);
        }
        AdminRequestCli::UninstallApp(args) => {
            let app_id = args.app_id.clone();
            let app = uninstall_app(cmd, args).await?;
            msg!("Uninstalled App: {}", app_id,);
        }
        AdminRequestCli::ListDnas => {
            let dnas = list_dnas(cmd).await?;
            msg!("Dnas: {:?}", dnas);
        }
        AdminRequestCli::NewAgent => {
            let agent = generate_agent_pub_key(cmd).await?;
            msg!("Added agent {}", agent);
        }
        AdminRequestCli::ListCells => {
            let cells = list_cell_ids(cmd).await?;
            msg!("Cell Ids: {:?}", cells);
        }
        AdminRequestCli::ListActiveApps => {
            let apps = list_running_apps(cmd).await?;
            msg!("Active Apps: {:?}", apps);
        }
        AdminRequestCli::ListApps(args) => {
            let apps = list_apps(cmd, args).await?;
            msg!("List Apps: {:?}", apps);
        }
        AdminRequestCli::EnableApp(args) => {
            let app_id = args.app_id.clone();
            enable_app(cmd, args).await?;
            msg!("Activated app: {:?}", app_id);
        }
        AdminRequestCli::DisableApp(args) => {
            let app_id = args.app_id.clone();
            disable_app(cmd, args).await?;
            msg!("Deactivated app: {:?}", app_id);
        }
        AdminRequestCli::DumpState(args) => {
            let state = dump_state(cmd, args).await?;
            msg!("DUMP STATE \n{}", state);
        }
        AdminRequestCli::AddAgents => todo!("Adding agent info via cli is coming soon"),
        AdminRequestCli::ListAgents(args) => {
            use std::fmt::Write;
            let agent_infos = request_agent_info(cmd, args).await?;
            for info in agent_infos {
                let mut out = String::new();
                let cell_info = list_cell_ids(cmd).await?;
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

                let this_agent = agents.iter().find(|a| *info.agent == a.1);
                let this_dna = dnas.iter().find(|d| *info.space == d.1).unwrap();
                if let Some(this_agent) = this_agent {
                    writeln!(out, "This Agent {:?} is {:?}", this_agent.0, this_agent.1)?;
                }
                writeln!(out, "This DNA {:?} is {:?}", this_dna.0, this_dna.1)?;

                use chrono::{DateTime, Duration, NaiveDateTime, Utc};
                let duration = Duration::milliseconds(info.signed_at_ms as i64);
                let s = duration.num_seconds() as i64;
                let n = duration.clone().to_std().unwrap().subsec_nanos();
                let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
                let duration = Duration::milliseconds(info.expires_at_ms as i64);
                let s = duration.num_seconds() as i64;
                let n = duration.clone().to_std().unwrap().subsec_nanos();
                let exp = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
                let now = Utc::now();

                writeln!(out, "signed at {}", dt)?;
                writeln!(
                    out,
                    "expires at {} in {}mins",
                    exp,
                    (exp - now).num_minutes()
                )?;
                writeln!(out, "space: {:?}", info.space)?;
                writeln!(out, "agent: {:?}", info.agent)?;
                writeln!(out, "urls: {:?}", info.url_list)?;
                msg!("{}\n", out);
            }
        }
    }
    Ok(())
}

/// Calls [`AdminRequest::AddAdminInterfaces`] and adds another admin interface.
pub async fn add_admin_interface(cmd: &mut CmdRunner, args: AddAdminWs) -> anyhow::Result<u16> {
    let port = args.port.unwrap_or(0);
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
    // TODO: return chosen port when 0 is used
    Ok(port)
}

/// Calls [`AdminRequest::RegisterDna`] and registers dna.
pub async fn register_dna(cmd: &mut CmdRunner, args: RegisterDna) -> anyhow::Result<DnaHash> {
    let RegisterDna {
        uid,
        properties,
        path,
        hash,
    } = args;
    let properties = match properties {
        Some(path) => Some(YamlProperties::new(serde_yaml::from_str(
            &std::fs::read_to_string(path)?,
        )?)),
        None => None,
    };
    let source = match (path, hash) {
        (None, Some(hash)) => DnaSource::Hash(hash),
        (Some(path), None) => DnaSource::Path(path),
        _ => unreachable!("Can't have hash and path for dna source"),
    };
    let dna = RegisterDnaPayload {
        uid,
        properties,
        source,
    };

    let r = AdminRequest::RegisterDna(Box::new(dna));
    let registered_dna = cmd.command(r).await?;
    let hash =
        expect_match!(registered_dna => AdminResponse::DnaRegistered, "Failed to register dna");
    Ok(hash)
}

/// Calls [`AdminRequest::InstallApp`] and installs a new app.
/// Creates an app per dna with the app id of `{app-id}-{dna-index}`
/// e.g. `my-cool-app-3`.
pub async fn install_app(
    cmd: &mut CmdRunner,
    args: InstallApp,
) -> anyhow::Result<InstalledAppInfo> {
    let InstallApp {
        app_id,
        agent_key,
        dnas,
    } = args;
    let agent_key = match agent_key {
        Some(agent) => agent,
        None => generate_agent_pub_key(cmd).await?,
    };

    let dnas = dnas
        .into_iter()
        .enumerate()
        .map(|(i, hash)| InstallAppDnaPayload::hash_only(hash, format!("{}-{}", app_id, i)))
        .collect();

    let app = InstallAppPayload {
        installed_app_id: app_id,
        agent_key,
        dnas,
    };

    let r = AdminRequest::InstallApp(app.into());
    let installed_app = cmd.command(r).await?;
    let installed_app =
        expect_match!(installed_app => AdminResponse::AppInstalled, "Failed to install app");
    enable_app(
        cmd,
        EnableApp {
            app_id: installed_app.installed_app_id.clone(),
        },
    )
    .await?;
    Ok(installed_app)
}

/// Calls [`AdminRequest::InstallApp`] and installs a new app.
pub async fn install_app_bundle(
    cmd: &mut CmdRunner,
    args: InstallAppBundle,
) -> anyhow::Result<InstalledAppInfo> {
    let InstallAppBundle {
        app_id,
        agent_key,
        path,
        uid,
    } = args;

    let bundle = AppBundleSource::Path(path).resolve().await?;

    let agent_key = match agent_key {
        Some(agent) => agent,
        None => generate_agent_pub_key(cmd).await?,
    };

    let payload = InstallAppBundlePayload {
        installed_app_id: app_id,
        agent_key,
        source: AppBundleSource::Bundle(bundle),
        membrane_proofs: Default::default(),
        uid,
    };

    let r = AdminRequest::InstallAppBundle(Box::new(payload));
    let installed_app = cmd.command(r).await?;
    let installed_app =
        expect_match!(installed_app => AdminResponse::AppBundleInstalled, "Failed to install app");
    enable_app(
        cmd,
        EnableApp {
            app_id: installed_app.installed_app_id.clone(),
        },
    )
    .await?;
    Ok(installed_app)
}

/// Calls [`AdminRequest::UninstallApp`] and uninstalls the installed app.
pub async fn uninstall_app(cmd: &mut CmdRunner, args: UninstallApp) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::UninstallApp {
            installed_app_id: args.app_id,
        })
        .await?;
    Ok(expect_match!(resp => AdminResponse::AppUninstalled, "Failed to uninstall app"))
}

/// Calls [`AdminRequest::ListAppInterfaces`].
pub async fn list_app_ws(cmd: &mut CmdRunner) -> anyhow::Result<Vec<u16>> {
    let resp = cmd.command(AdminRequest::ListAppInterfaces).await?;
    Ok(expect_match!(resp => AdminResponse::AppInterfacesListed, "Failed to list app interfaces"))
}

/// Calls [`AdminRequest::ListCellIds`].
pub async fn list_dnas(cmd: &mut CmdRunner) -> anyhow::Result<Vec<DnaHash>> {
    let resp = cmd.command(AdminRequest::ListDnas).await?;
    Ok(expect_match!(resp => AdminResponse::DnasListed, "Failed to list dnas"))
}

/// Calls [`AdminRequest::GenerateAgentPubKey`].
pub async fn generate_agent_pub_key(cmd: &mut CmdRunner) -> anyhow::Result<AgentPubKey> {
    let resp = cmd.command(AdminRequest::GenerateAgentPubKey).await?;
    Ok(
        expect_match!(resp => AdminResponse::AgentPubKeyGenerated, "Failed to generate agent pubkey"),
    )
}

/// Calls [`AdminRequest::ListCellIds`].
pub async fn list_cell_ids(cmd: &mut CmdRunner) -> anyhow::Result<Vec<CellId>> {
    let resp = cmd.command(AdminRequest::ListCellIds).await?;
    Ok(expect_match!(resp => AdminResponse::CellIdsListed, "Failed to list cell ids"))
}

/// Calls [`AdminRequest::ListActiveApps`].
pub async fn list_running_apps(cmd: &mut CmdRunner) -> anyhow::Result<Vec<String>> {
    let resp = cmd.command(AdminRequest::ListActiveApps).await?;
    Ok(expect_match!(resp => AdminResponse::ActiveAppsListed, "Failed to list active apps"))
}

/// Calls [`AdminRequest::ListApps`].
pub async fn list_apps(
    cmd: &mut CmdRunner,
    args: ListApps,
) -> anyhow::Result<Vec<InstalledAppInfo>> {
    let resp = cmd
        .command(AdminRequest::ListApps {
            status_filter: args.status,
        })
        .await?;
    Ok(expect_match!(resp => AdminResponse::AppsListed, "Failed to list apps"))
}

/// Calls [`AdminRequest::EnableApp`] and activates the installed app.
pub async fn enable_app(cmd: &mut CmdRunner, args: EnableApp) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::EnableApp {
            installed_app_id: args.app_id,
        })
        .await?;
    assert!(matches!(resp, AdminResponse::AppEnabled { .. }));
    Ok(())
}

/// Calls [`AdminRequest::DisableApp`] and disables the installed app.
pub async fn disable_app(cmd: &mut CmdRunner, args: DisableApp) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::DisableApp {
            installed_app_id: args.app_id,
        })
        .await?;
    ensure!(
        matches!(resp, AdminResponse::AppDisabled),
        "Failed to disable app, got: {:?}",
        resp
    );
    Ok(())
}

/// Calls [`AdminRequest::AttachAppInterface`] and adds another app interface.
pub async fn attach_app_interface(cmd: &mut CmdRunner, args: AddAppWs) -> anyhow::Result<u16> {
    let resp = cmd
        .command(AdminRequest::AttachAppInterface { port: args.port })
        .await?;
    tracing::debug!(?resp);
    match resp {
        AdminResponse::AppInterfaceAttached { port } => Ok(port),
        _ => Err(anyhow!(
            "Failed to attach app interface {:?}, got: {:?}",
            args.port,
            resp
        )),
    }
}

/// Calls [`AdminRequest::DumpState`] and dumps the current cell's state.
// TODO: Add pretty print.
// TODO: Default to dumping all cell state.
pub async fn dump_state(cmd: &mut CmdRunner, args: DumpState) -> anyhow::Result<String> {
    let resp = cmd
        .command(AdminRequest::DumpState {
            cell_id: Box::new(args.into()),
        })
        .await?;
    Ok(expect_match!(resp => AdminResponse::StateDumped, "Failed to dump state"))
}

/// Calls [`AdminRequest::AddAgentInfo`] with and adds the list of agent info.
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

/// Calls [`AdminRequest::RequestAgentInfo`] and pretty prints the agent info on this conductor.
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

fn parse_status_filter(arg: &str) -> anyhow::Result<AppStatusFilter> {
    match arg {
        "active" => Ok(AppStatusFilter::Enabled),
        "inactive" => Ok(AppStatusFilter::Disabled),
        _ => Err(anyhow::anyhow!(
            "Bad app status filter value: {}, only 'active' and 'inactive' are possible",
            arg
        )),
    }
}

impl From<CellId> for DumpState {
    fn from(cell_id: CellId) -> Self {
        let (dna, agent_key) = cell_id.into_dna_and_agent();
        Self { dna, agent_key }
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
