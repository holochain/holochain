//! Helpers for making [`AdminRequest`]s to the admin API.
//!
//! This module is designed for use in a CLI so it is more simplified
//! than calling the [`CmdRunner`] directly.
//! For simple calls like [`AdminRequest::ListDnas`] this is probably easier
//! but if you want more control use [`CmdRunner::command`].

use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::AdminResponse;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::InterfaceDriver;
use holochain_conductor_api::{AdminInterfaceConfig, AppInfo};
use holochain_conductor_api::{AdminRequest, AppInterfaceInfo};
use holochain_types::prelude::DnaModifiersOpt;
use holochain_types::prelude::RegisterDnaPayload;
use holochain_types::prelude::Timestamp;
use holochain_types::prelude::YamlProperties;
use holochain_types::prelude::{AgentPubKey, AppBundleSource};
use holochain_types::prelude::{CellId, InstallAppPayload};
use holochain_types::prelude::{DnaHash, InstalledAppId};
use holochain_types::prelude::{DnaSource, NetworkSeed};
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use std::convert::TryFrom;

use crate::cmds::Existing;
use crate::expect_match;
use crate::ports::get_admin_ports;
use crate::run::run_async;
use crate::CmdRunner;
use clap::{Args, Parser, Subcommand};
use holochain_trace::Output;
use holochain_types::websocket::AllowedOrigins;

#[doc(hidden)]
#[derive(Debug, Parser)]
pub struct Call {
    /// Ports to running conductor admin interfaces.
    /// If this is empty existing sandboxes will be used.
    /// Cannot be combined with existing sandboxes.
    #[arg(short, long, conflicts_with_all = &["existing_paths", "indices"], value_delimiter = ',')]
    pub running: Vec<u16>,

    #[command(flatten)]
    pub existing: Existing,

    /// The admin request you want to make.
    #[command(subcommand)]
    pub call: AdminRequestCli,
}

// Docs have different use for clap
// so documenting everything doesn't make sense.
#[allow(missing_docs)]
#[derive(Debug, Subcommand, Clone)]
pub enum AdminRequestCli {
    AddAdminWs(AddAdminWs),
    AddAppWs(AddAppWs),
    RegisterDna(RegisterDna),
    InstallApp(InstallApp),
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
    /// Calls AdminRequest::ListApps.
    ListApps(ListApps),
    EnableApp(EnableApp),
    DisableApp(DisableApp),
    DumpState(DumpState),
    DumpConductorState,
    DumpNetworkMetrics(DumpNetworkMetrics),
    DumpNetworkStats,
    /// Calls AdminRequest::AddAgentInfo.
    /// _Unimplemented_.
    AddAgents,
    ListAgents(ListAgents),
}

/// Calls AdminRequest::AddAdminInterfaces
/// and adds another admin interface.
#[derive(Debug, Args, Clone)]
pub struct AddAdminWs {
    /// Optional port number.
    /// Defaults to assigned by OS.
    pub port: Option<u16>,

    /// Optional allowed origins.
    ///
    /// This should be a comma separated list of origins, or `*` to allow any origin.
    /// For example: `http://localhost:3000,http://localhost:3001`
    ///
    /// If not provided, defaults to `*` which allows any origin.
    #[arg(long, default_value_t = AllowedOrigins::Any)]
    pub allowed_origins: AllowedOrigins,
}

/// Calls AdminRequest::AttachAppInterface
/// and adds another app interface.
#[derive(Debug, Args, Clone)]
pub struct AddAppWs {
    /// Optional port number.
    /// Defaults to assigned by OS.
    pub port: Option<u16>,

    /// Optional allowed origins.
    ///
    /// This should be a comma separated list of origins, or `*` to allow any origin.
    /// For example: `http://localhost:3000,http://localhost:3001`
    ///
    /// If not provided, defaults to `*` which allows any origin.
    #[arg(long, default_value_t = AllowedOrigins::Any)]
    pub allowed_origins: AllowedOrigins,

    /// Optional app id to restrict this interface to.
    ///
    /// If provided then only apps with an authentication token issued to the same app id
    /// will be allowed to connect to this interface.
    #[arg(long)]
    pub installed_app_id: Option<InstalledAppId>,
}

/// Calls AdminRequest::RegisterDna
/// and registers a DNA. You can only use a path or a hash, not both.
#[derive(Debug, Args, Clone)]
pub struct RegisterDna {
    #[arg(short, long)]
    /// Network seed to override when installing this DNA
    pub network_seed: Option<String>,
    #[arg(long)]
    /// Properties to override when installing this DNA
    pub properties: Option<PathBuf>,
    #[arg(long)]
    /// Origin time to override when installing this DNA
    pub origin_time: Option<Timestamp>,
    #[arg(long, conflicts_with = "hash", required_unless_present = "hash")]
    /// Path to a DnaBundle file.
    pub path: Option<PathBuf>,
    #[arg(short, long, value_parser = parse_dna_hash, required_unless_present = "path")]
    /// Hash of an existing DNA you want to register.
    pub hash: Option<DnaHash>,
}

/// Calls AdminRequest::InstallApp
/// and installs a new app.
///
/// Setting properties and membrane proofs is not
/// yet supported.
/// RoleNames are set to `my-app-0`, `my-app-1` etc.
#[derive(Debug, Args, Clone)]
pub struct InstallApp {
    /// Sets the InstalledAppId.
    #[arg(long)]
    pub app_id: Option<String>,

    /// If not set then a key will be generated.
    /// Agent key is Base64 (same format that is used in logs).
    /// e.g. `uhCAk71wNXTv7lstvi4PfUr_JDvxLucF9WzUgWPNIEZIoPGMF4b_o`
    #[arg(long, value_parser = parse_agent_key)]
    pub agent_key: Option<AgentPubKey>,

    /// Location of the *.happ bundle file to install.
    #[arg(required = true)]
    pub path: PathBuf,

    /// Optional network seed override for every DNA in this app
    pub network_seed: Option<NetworkSeed>,
}

/// Calls AdminRequest::UninstallApp
/// and uninstalls the specified app.
#[derive(Debug, Args, Clone)]
pub struct UninstallApp {
    /// The InstalledAppId to uninstall.
    pub app_id: String,
}

/// Calls AdminRequest::EnableApp
/// and activates the installed app.
#[derive(Debug, Args, Clone)]
pub struct EnableApp {
    /// The InstalledAppId to activate.
    pub app_id: String,
}

/// Calls AdminRequest::DisableApp
/// and disables the installed app.
#[derive(Debug, Args, Clone)]
pub struct DisableApp {
    /// The InstalledAppId to disable.
    pub app_id: String,
}

/// Calls AdminRequest::DumpState
/// and dumps the current cell's state.
// TODO: Add pretty print.
// TODO: Default to dumping all cell state.
#[derive(Debug, Args, Clone)]
pub struct DumpState {
    /// The DNA hash half of the cell ID to dump.
    #[arg(value_parser = parse_dna_hash)]
    pub dna: DnaHash,

    /// The agent half of the cell ID to dump.
    #[arg(value_parser = parse_agent_key)]
    pub agent_key: AgentPubKey,
}

/// Arguments for dumping network metrics.
#[derive(Debug, Args, Clone)]
pub struct DumpNetworkMetrics {
    /// The DNA hash of the app network to dump.
    #[arg(value_parser = parse_dna_hash)]
    pub dna: Option<DnaHash>,
}

/// Calls AdminRequest::RequestAgentInfo
/// and pretty prints the agent info on
/// this conductor.
#[derive(Debug, Args, Clone)]
pub struct ListAgents {
    /// Optionally request agent info for a particular cell ID.
    #[arg(short, long, value_parser = parse_agent_key, requires = "dna")]
    pub agent_key: Option<AgentPubKey>,

    /// Optionally request agent info for a particular cell ID.
    #[arg(short, long, value_parser = parse_dna_hash, requires = "agent_key")]
    pub dna: Option<DnaHash>,
}

/// Calls AdminRequest::ListApps
/// and pretty prints the list of apps
/// installed in this conductor.
#[derive(Debug, Args, Clone)]
pub struct ListApps {
    /// Optionally request agent info for a particular cell ID.
    #[arg(short, long, value_parser = parse_status_filter)]
    pub status: Option<AppStatusFilter>,
}

#[doc(hidden)]
pub async fn call(
    holochain_path: &Path,
    req: Call,
    force_admin_ports: Vec<u16>,
    structured: Output,
) -> anyhow::Result<()> {
    let Call {
        existing,
        running,
        call,
    } = req;
    // Force admin ports takes precedence over running. They both specify the same thing but force admin ports
    // is used across other sandbox calls so this makes `call` consistent with others.
    let running = if force_admin_ports.is_empty() {
        running
    } else {
        force_admin_ports
    };

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
                Ok(cmd) => cmds.push((cmd, None, None)),
                Err(e) => {
                    if let std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::AddrNotAvailable = e.kind()
                    {
                        let (port, holochain, lair) = run_async(
                            holochain_path,
                            ConfigRootPath::from(path),
                            None,
                            structured.clone(),
                        )
                        .await?;
                        cmds.push((CmdRunner::new(port).await, Some(holochain), Some(lair)));
                        continue;
                    }
                    bail!(
                        "Failed to connect to running conductor or start one {:?}",
                        e
                    )
                }
            }
        }

        if cmds.is_empty() {
            bail!(
                "No running conductors found by searching the current directory. \
                \nYou need to do one of: \
                    \n\t1. Start a new sandbox conductor from this directory, \
                    \n\t2. Change directory to where your sandbox conductor is running, \
                    \n\t3. Use the --running flag to connect to a running conductor\
                "
            );
        }

        cmds
    } else {
        let mut cmds = Vec::with_capacity(running.len());
        for port in running {
            cmds.push((CmdRunner::new(port).await, None, None));
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
            msg!("Added admin port {}", port);
        }
        AdminRequestCli::AddAppWs(args) => {
            let port = attach_app_interface(cmd, args).await?;
            msg!("Added app port {}", port);
        }
        AdminRequestCli::ListAppWs => {
            let ports = list_app_ws(cmd).await?;
            msg!("Attached app interfaces {:?}", ports);
        }
        AdminRequestCli::RegisterDna(args) => {
            let dnas = register_dna(cmd, args).await?;
            msg!("Registered DNA: {:?}", dnas);
        }
        AdminRequestCli::InstallApp(args) => {
            let app = install_app_bundle(cmd, args).await?;
            msg!("Installed app: {}", app.installed_app_id,);
        }
        AdminRequestCli::UninstallApp(args) => {
            let app_id = args.app_id.clone();
            uninstall_app(cmd, args).await?;
            msg!("Uninstalled app: {}", app_id,);
        }
        AdminRequestCli::ListDnas => {
            let dnas = list_dnas(cmd).await?;
            msg!("DNAs: {:?}", dnas);
        }
        AdminRequestCli::NewAgent => {
            let agent = generate_agent_pub_key(cmd).await?;
            msg!("Added agent {}", agent);
        }
        AdminRequestCli::ListCells => {
            let cells = list_cell_ids(cmd).await?;
            msg!("Cell IDs: {:?}", cells);
        }
        AdminRequestCli::ListApps(args) => {
            let apps = list_apps(cmd, args).await?;
            msg!("List apps: {:?}", apps);
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
        AdminRequestCli::DumpConductorState => {
            let state = dump_conductor_state(cmd).await?;
            msg!("DUMP CONDUCTOR STATE \n{}", state);
        }
        AdminRequestCli::DumpNetworkMetrics(args) => {
            let metrics = dump_network_metrics(cmd, args).await?;
            // Print without other text so it can be piped
            println!("{}", metrics);
        }
        AdminRequestCli::DumpNetworkStats => {
            let stats = dump_network_stats(cmd).await?;
            // Print without other text so it can be piped
            println!("{}", stats);
        }
        AdminRequestCli::AddAgents => todo!("Adding agent info via CLI is coming soon"),
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
                    writeln!(out, "This agent {:?} is {:?}", this_agent.0, this_agent.1)?;
                }
                writeln!(out, "This DNA {:?} is {:?}", this_dna.0, this_dna.1)?;

                use chrono::{DateTime, Duration, NaiveDateTime, Utc};
                let duration = Duration::try_milliseconds(info.signed_at_ms as i64)
                    .ok_or_else(|| anyhow!("Agent info timestamp out of range"))?;
                let s = duration.num_seconds();
                let n = duration.clone().to_std().unwrap().subsec_nanos();
                // TODO FIXME
                #[allow(deprecated)]
                let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
                let duration = Duration::try_milliseconds(info.expires_at_ms as i64)
                    .ok_or_else(|| anyhow!("Agent info timestamp out of range"))?;
                let s = duration.num_seconds();
                let n = duration.clone().to_std().unwrap().subsec_nanos();
                // TODO FIXME
                #[allow(deprecated)]
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
                writeln!(out, "URLs: {:?}", info.url_list)?;
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
                driver: InterfaceDriver::Websocket {
                    port,
                    allowed_origins: AllowedOrigins::Any,
                },
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

/// Calls [`AdminRequest::RegisterDna`] and registers DNA.
pub async fn register_dna(cmd: &mut CmdRunner, args: RegisterDna) -> anyhow::Result<DnaHash> {
    let RegisterDna {
        network_seed,
        properties,
        origin_time,
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
        _ => unreachable!("Can't have hash and path for DNA source"),
    };
    let dna = RegisterDnaPayload {
        modifiers: DnaModifiersOpt {
            properties,
            network_seed,
            origin_time,
            quantum_time: None,
        },
        source,
    };

    let r = AdminRequest::RegisterDna(Box::new(dna));
    let registered_dna = cmd.command(r).await?;
    let hash =
        expect_match!(registered_dna => AdminResponse::DnaRegistered, "Failed to register DNA");
    Ok(hash)
}

/// Calls [`AdminRequest::InstallApp`] and installs a new app.
pub async fn install_app_bundle(cmd: &mut CmdRunner, args: InstallApp) -> anyhow::Result<AppInfo> {
    let InstallApp {
        app_id,
        agent_key,
        path,
        network_seed,
    } = args;

    let agent_key = match agent_key {
        Some(agent) => agent,
        None => generate_agent_pub_key(cmd).await?,
    };

    let payload = InstallAppPayload {
        installed_app_id: app_id,
        agent_key,
        source: AppBundleSource::Path(path),
        membrane_proofs: Default::default(),
        network_seed,
        #[cfg(feature = "chc")]
        ignore_genesis_failure: false,
    };

    let r = AdminRequest::InstallApp(Box::new(payload));
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

/// Calls [`AdminRequest::UninstallApp`] and uninstalls the installed app.
pub async fn uninstall_app(cmd: &mut CmdRunner, args: UninstallApp) -> anyhow::Result<()> {
    let resp = cmd
        .command(AdminRequest::UninstallApp {
            installed_app_id: args.app_id,
        })
        .await?;

    assert!(
        matches!(resp, AdminResponse::AppUninstalled),
        "Failed to uninstall app"
    );
    Ok(())
}

/// Calls [`AdminRequest::ListAppInterfaces`].
pub async fn list_app_ws(cmd: &mut CmdRunner) -> anyhow::Result<Vec<AppInterfaceInfo>> {
    let resp = cmd.command(AdminRequest::ListAppInterfaces).await?;
    Ok(expect_match!(resp => AdminResponse::AppInterfacesListed, "Failed to list app interfaces"))
}

/// Calls [`AdminRequest::ListCellIds`].
pub async fn list_dnas(cmd: &mut CmdRunner) -> anyhow::Result<Vec<DnaHash>> {
    let resp = cmd.command(AdminRequest::ListDnas).await?;
    Ok(expect_match!(resp => AdminResponse::DnasListed, "Failed to list DNAs"))
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
    Ok(expect_match!(resp => AdminResponse::CellIdsListed, "Failed to list cell IDs"))
}

/// Calls [`AdminRequest::ListApps`].
pub async fn list_apps(cmd: &mut CmdRunner, args: ListApps) -> anyhow::Result<Vec<AppInfo>> {
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
        .command(AdminRequest::AttachAppInterface {
            port: args.port,
            allowed_origins: args.allowed_origins,
            installed_app_id: args.installed_app_id,
        })
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

/// Calls [`AdminRequest::DumpConductorState`] and dumps the current conductor state.
pub async fn dump_conductor_state(cmd: &mut CmdRunner) -> anyhow::Result<String> {
    let resp = cmd.command(AdminRequest::DumpConductorState).await?;
    Ok(expect_match!(resp => AdminResponse::ConductorStateDumped, "Failed to dump state"))
}

/// Calls [`AdminRequest::DumpNetworkMetrics`] and dumps network metrics.
async fn dump_network_metrics(
    cmd: &mut CmdRunner,
    args: DumpNetworkMetrics,
) -> anyhow::Result<String> {
    let resp = cmd
        .command(AdminRequest::DumpNetworkMetrics { dna_hash: args.dna })
        .await?;
    Ok(
        expect_match!(resp => AdminResponse::NetworkMetricsDumped, "Failed to dump network metrics"),
    )
}

/// Calls [`AdminRequest::DumpNetworkStats`] and dumps network stats.
async fn dump_network_stats(cmd: &mut CmdRunner) -> anyhow::Result<String> {
    let resp = cmd.command(AdminRequest::DumpNetworkStats).await?;
    Ok(
        expect_match!(resp => AdminResponse::NetworkStatsDumped, "Failed to dump network stats"),
    )

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

/// Calls [`AdminRequest::AgentInfo`] and pretty prints the agent info on this conductor.
pub async fn request_agent_info(
    cmd: &mut CmdRunner,
    args: ListAgents,
) -> anyhow::Result<Vec<AgentInfoSigned>> {
    let resp = cmd
        .command(AdminRequest::AgentInfo {
            cell_id: args.into(),
        })
        .await?;
    Ok(expect_match!(resp => AdminResponse::AgentInfo, "Failed to request agent info"))
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
