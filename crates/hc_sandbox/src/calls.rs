//! Helpers for making [`holochain_conductor_api::AdminRequest`]s to the admin API.
//!
//! This module is designed for use in a CLI so it is more simplified
//! than calling the [`AdminWebsocket`] directly.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use anyhow::bail;
use holo_hash::ActionHash;
use holo_hash::DnaHashB64;
use holochain_client::AdminWebsocket;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::AgentMetaInfo;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::InterfaceDriver;
use holochain_conductor_api::{AdminInterfaceConfig, AppInfo};
use holochain_types::app::AppManifest;
use holochain_types::app::RoleSettingsMap;
use holochain_types::app::RoleSettingsMapYaml;
use holochain_types::prelude::DnaModifiersOpt;
use holochain_types::prelude::RegisterDnaPayload;
use holochain_types::prelude::YamlProperties;
use holochain_types::prelude::{AgentPubKey, AppBundleSource};
use holochain_types::prelude::{CellId, InstallAppPayload};
use holochain_types::prelude::{DnaHash, InstalledAppId};
use holochain_types::prelude::{DnaSource, NetworkSeed};
use kitsune2_api::Url;
use kitsune2_core::Ed25519Verifier;
use std::convert::TryFrom;

use crate::cmds::Existing;
use crate::ports::get_admin_ports;
use crate::run::run_async;
use clap::{Args, Parser, Subcommand};
use holochain_trace::Output;
use holochain_types::websocket::AllowedOrigins;
use kitsune2_api::AgentInfoSigned;

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

    /// The origin to use in the admin websocket request
    #[arg(long)]
    pub origin: Option<String>,

    /// The admin request you want to make.
    #[command(subcommand)]
    pub call: AdminRequestCli,
}

// Docs have different use for clap
// so documenting everything doesn't make sense.
#[derive(Debug, Subcommand, Clone)]
pub enum AdminRequestCli {
    /// Calls [`AdminWebsocket::add_admin_interfaces`].
    AddAdminWs(AddAdminWs),
    /// Calls [`AdminWebsocket::attach_app_interface`].
    AddAppWs(AddAppWs),
    /// Calls [`AdminWebsocket::register_dna`].
    RegisterDna(RegisterDna),
    /// Calls [`AdminWebsocket::install_app`].
    InstallApp(InstallApp),
    /// Calls [`AdminWebsocket::uninstall_app`].
    UninstallApp(UninstallApp),
    /// Calls [`AdminWebsocket::list_app_interfaces`].
    ListAppWs,
    /// Calls [`AdminWebsocket::list_dnas`].
    ListDnas,
    /// Calls [`AdminWebsocket::generate_agent_pub_key`].
    NewAgent,
    /// Calls [`AdminWebsocket::list_cell_ids`].
    ListCells,
    /// Calls [`AdminWebsocket::list_apps`].
    ListApps(ListApps),
    /// Calls [`AdminWebsocket::enable_app`].
    EnableApp(EnableApp),
    /// Calls [`AdminWebsocket::disable_app`].
    DisableApp(DisableApp),
    /// Calls [`AdminWebsocket::dump_state`].
    DumpState(DumpState),
    /// Calls [`AdminWebsocket::dump_conductor_state`].
    DumpConductorState,
    /// Calls [`AdminWebsocket::dump_network_metrics`].
    DumpNetworkMetrics(DumpNetworkMetrics),
    /// Calls [`AdminWebsocket::dump_network_stats`].
    DumpNetworkStats,
    /// Calls [`AdminWebsocket::list_capability_grants`].
    ListCapabilityGrants(ListCapGrants),
    /// Calls [`AdminWebsocket::revoke_zome_call_capability`].
    RevokeZomeCallCapability(RevokeZomeCallCapability),
    /// Calls [`AdminWebsocket::add_agent_info`].
    AddAgents(AgentInfos),
    /// Calls [`AdminWebsocket::agent_info`].
    ListAgents(ListAgents),
    /// Calls [`AdminWebsocket::agent_meta_info`].
    AgentMetaInfo(AgentMetaInfoArgs),
}

/// Calls [`AdminWebsocket::add_admin_interfaces`]
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

/// Calls [`AdminWebsocket::attach_app_interface`]
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

/// Calls [`AdminWebsocket::register_dna`]
/// and registers a DNA. You can only use a path or a hash, not both.
#[derive(Debug, Args, Clone)]
pub struct RegisterDna {
    #[arg(short, long)]
    /// Network seed to override when installing this DNA
    pub network_seed: Option<String>,
    #[arg(long)]
    /// Properties to override when installing this DNA
    pub properties: Option<PathBuf>,
    #[arg(long, conflicts_with = "hash", required_unless_present = "hash")]
    /// Path to a DnaBundle file.
    pub path: Option<PathBuf>,
    #[arg(short, long, value_parser = parse_dna_hash, required_unless_present = "path")]
    /// Hash of an existing DNA you want to register.
    pub hash: Option<DnaHash>,
}

/// Calls [`AdminWebsocket::install_app`]
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

    /// Optional path to a yaml file containing role settings to override
    /// the values in the dna manifest(s).
    /// See <https://github.com/holochain/holochain/tree/develop/crates/hc_sandbox/tests/fixtures/roles-settings.yaml>
    /// for an example of such a yaml file.
    pub roles_settings: Option<PathBuf>,
}

/// Calls [`AdminWebsocket::uninstall_app`]
/// and uninstalls the specified app.
#[derive(Debug, Args, Clone)]
pub struct UninstallApp {
    /// The InstalledAppId to uninstall.
    pub app_id: String,

    /// Force uninstallation of the app even if there are any protections in place.
    ///
    /// Possible protections:
    /// - Another app depends on a cell in the app you are trying to uninstall.
    ///
    /// Please check that you understand the consequences of forcing the uninstall before using this option.
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

/// Calls [`AdminWebsocket::enable_app`]
/// and activates the installed app.
#[derive(Debug, Args, Clone)]
pub struct EnableApp {
    /// The InstalledAppId to activate.
    pub app_id: String,
}

/// Calls [`AdminWebsocket::disable_app`]
/// and disables the installed app.
#[derive(Debug, Args, Clone)]
pub struct DisableApp {
    /// The InstalledAppId to disable.
    pub app_id: String,
}

/// Calls [`AdminWebsocket::dump_state`]
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

    /// Include DHT summary in the response.
    #[arg(long)]
    pub include_dht_summary: bool,
}

/// Arguments for listing capability grants info.
#[derive(Debug, Args, Clone)]
pub struct ListCapGrants {
    /// app id to filter by
    pub installed_app_id: String,
    /// include revoked grants
    pub include_revoked: bool,
}

/// Arguments for revoking a zome call capability.
#[derive(Debug, Args, Clone)]
pub struct RevokeZomeCallCapability {
    /// The [`ActionHash`] of the zome call capability to revoke.
    pub action_hash: String,
}

/// Calls [`AdminWebsocket::add_agent_info`]
/// and disables the installed app.
#[derive(Debug, Args, Clone)]
pub struct AgentInfos {
    /// A JSON array of agent infos.
    pub agent_infos: String,
}

/// Calls [`AdminWebsocket::agent_info`]
/// and pretty prints the agent info on
/// this conductor.
#[derive(Debug, Args, Clone)]
pub struct ListAgents {
    /// Optionally request agent info for a list of DNA hashes.
    #[arg(short, long, num_args = 0.., value_parser = parse_dna_hash)]
    pub dna: Option<Vec<DnaHash>>,
}

/// Calls [`AdminWebsocket::list_apps`]
/// and pretty prints the list of apps
/// installed in this conductor.
#[derive(Debug, Args, Clone)]
pub struct ListApps {
    /// Optionally request agent info for a particular cell ID.
    #[arg(short, long, value_parser = parse_status_filter)]
    pub status: Option<AppStatusFilter>,
}

/// Calls [`AdminWebsocket::agent_meta_info`]
/// and prints the agent meta info related to the specified Url
#[derive(Debug, Args, Clone)]
pub struct AgentMetaInfoArgs {
    /// The kitsune Url of the agent to get meta info about.
    #[arg(long)]
    pub url: Url,

    /// Optionally request agent meta info for a list of DNA hashes.
    #[arg(short, long, num_args = 0.., value_parser = parse_dna_hash)]
    pub dna: Option<Vec<DnaHash>>,
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
        origin,
        call,
    } = req;
    // Force admin ports takes precedence over running. They both specify the same thing but force admin ports
    // is used across other sandbox calls so this makes `call` consistent with others.
    let running = if force_admin_ports.is_empty() {
        running
    } else {
        force_admin_ports
    };

    let admin_clients = if running.is_empty() {
        let paths = if existing.is_empty() {
            crate::save::load(std::env::current_dir()?)?
        } else {
            existing.load()?
        };
        let ports = get_admin_ports(paths.clone()).await?;
        let mut clients = Vec::with_capacity(ports.len());
        for (port, path) in ports.into_iter().zip(paths.into_iter()) {
            match AdminWebsocket::connect(format!("localhost:{port}"), origin.clone()).await {
                Ok(client) => clients.push((client, None, None)),
                Err(e) => {
                    tracing::debug!("Connecting to the sandbox conductor failed: {e}.\nThis is expected in case the conductor is not running. Trying to start it up now...");
                    // Note that the holochain and lair processes need to be returned here
                    // in order to not get dropped but keep running until the admin call
                    // is being made
                    let (port, holochain, lair) = run_async(
                        holochain_path,
                        ConfigRootPath::from(path),
                        None,
                        structured.clone(),
                    )
                    .await?;
                    clients.push((
                        AdminWebsocket::connect(format!("localhost:{port}"), origin.clone())
                            .await?,
                        Some(holochain),
                        Some(lair),
                    ));
                }
            }
        }

        if clients.is_empty() {
            bail!(
                "No running conductors found by searching the current directory. \
                \nYou need to do one of: \
                    \n\t1. Start a new sandbox conductor from this directory, \
                    \n\t2. Change directory to where your sandbox conductor is running, \
                    \n\t3. Use the --running flag to connect to a running conductor\
                "
            );
        }

        clients
    } else {
        let mut clients = Vec::with_capacity(running.len());
        for port in running {
            clients.push((
                AdminWebsocket::connect(format!("localhost:{port}"), origin.clone()).await?,
                None,
                None,
            ));
        }
        clients
    };
    for mut client in admin_clients {
        call_inner(&mut client.0, call.clone()).await?;
    }
    Ok(())
}

async fn call_inner(client: &mut AdminWebsocket, call: AdminRequestCli) -> anyhow::Result<()> {
    match call {
        AdminRequestCli::AddAdminWs(args) => {
            let port = args.port.unwrap_or(0);
            client
                .add_admin_interfaces(vec![AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket {
                        port,
                        allowed_origins: args.allowed_origins,
                    },
                }])
                .await?;
            msg!("Added admin port {}", port);
        }
        AdminRequestCli::AddAppWs(args) => {
            let port = args.port.unwrap_or(0);
            let port = client
                .attach_app_interface(port, args.allowed_origins, args.installed_app_id)
                .await?;
            msg!("Added app port {}", port);
        }
        AdminRequestCli::ListAppWs => {
            let interface_infos = client.list_app_interfaces().await?;
            msg!("Attached app interfaces {:?}", interface_infos);
        }
        AdminRequestCli::RegisterDna(args) => {
            let dnas = register_dna(client, args).await?;
            msg!("Registered DNA: {:?}", dnas);
        }
        AdminRequestCli::InstallApp(args) => {
            let app = install_app_bundle(client, args).await?;
            msg!("Installed app: {}", app.installed_app_id,);
        }
        AdminRequestCli::UninstallApp(args) => {
            client
                .uninstall_app(args.app_id.clone(), args.force)
                .await?;
            msg!("Uninstalled app: {}", args.app_id,);
        }
        AdminRequestCli::ListDnas => {
            let dnas = client.list_dnas().await?;
            msg!("DNAs: {:?}", dnas);
        }
        AdminRequestCli::NewAgent => {
            let agent = client.generate_agent_pub_key().await?;
            msg!("Added agent {}", agent);
        }
        AdminRequestCli::ListCells => {
            let cells = client.list_cell_ids().await?;
            msg!("Cell IDs: {:?}", cells);
        }
        AdminRequestCli::ListApps(args) => {
            let apps = client.list_apps(args.status).await?;
            msg!("List apps: {:?}", apps);
        }
        AdminRequestCli::EnableApp(args) => {
            client.enable_app(args.app_id.clone()).await?;
            msg!("Activated app: {:?}", args.app_id);
        }
        AdminRequestCli::DisableApp(args) => {
            client.disable_app(args.app_id.clone()).await?;
            msg!("Deactivated app: {:?}", args.app_id);
        }
        AdminRequestCli::DumpState(args) => {
            let state = client.dump_state(args.into()).await?;
            msg!("DUMP STATE \n{}", state);
        }
        AdminRequestCli::DumpConductorState => {
            let state = client.dump_conductor_state().await?;
            msg!("DUMP CONDUCTOR STATE \n{}", state);
        }
        AdminRequestCli::DumpNetworkMetrics(args) => {
            let metrics = client
                .dump_network_metrics(args.dna, args.include_dht_summary)
                .await?;
            // Print without other text so it can be piped
            println!(
                "{}",
                serde_json::to_string(
                    &metrics
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect::<HashMap<_, _>>()
                )?
            );
        }
        AdminRequestCli::DumpNetworkStats => {
            let stats = client.dump_network_stats().await?;
            // Print without other text so it can be piped
            println!("{}", serde_json::to_string(&stats)?);
        }
        AdminRequestCli::RevokeZomeCallCapability(args) => {
            let action_hash = ActionHash::try_from(&args.action_hash)
                .map_err(|e| anyhow!("Invalid action hash: {}", e))?;
            client.revoke_zome_call_capability(action_hash).await?;
            msg!(
                "Revoked zome call capability for action hash: {}",
                args.action_hash
            );
        }
        AdminRequestCli::ListCapabilityGrants(args) => {
            let info = client
                .list_capability_grants(args.installed_app_id, args.include_revoked)
                .await?;
            // Print without other text so it can be piped
            println!("{:?}", info);
        }
        AdminRequestCli::AddAgents(args) => {
            let agent_infos_results =
                AgentInfoSigned::decode_list(&Ed25519Verifier, args.agent_infos.as_bytes())?;
            let agent_infos = agent_infos_results
                .into_iter()
                .map(|r| r.expect("Failed to decode agent info."))
                .collect();
            add_agent_info(client, agent_infos).await?;
        }
        AdminRequestCli::ListAgents(args) => {
            use std::fmt::Write;
            let agent_infos = request_agent_info(client, args).await?;
            for info in agent_infos {
                let mut out = String::new();
                let cell_info = client.list_cell_ids().await?;
                let agents = cell_info
                    .iter()
                    .map(|c| c.agent_pubkey().clone())
                    .map(|a| (a.clone(), a.to_k2_agent()))
                    .collect::<Vec<_>>();

                let dnas = cell_info
                    .iter()
                    .map(|c| c.dna_hash().clone())
                    .map(|d| (d.clone(), d.to_k2_space()))
                    .collect::<Vec<_>>();

                let this_agent = agents.iter().find(|a| info.agent == a.1);
                let this_dna = dnas.iter().find(|d| info.space == d.1).unwrap();
                if let Some(this_agent) = this_agent {
                    writeln!(out, "This agent {:?} is {:?}", this_agent.0, this_agent.1)?;
                }
                writeln!(out, "This DNA {:?} is {:?}", this_dna.0, this_dna.1)?;

                use chrono::{DateTime, Duration, NaiveDateTime, Utc};
                let duration = Duration::try_milliseconds(info.created_at.as_micros() / 1000)
                    .ok_or_else(|| anyhow!("Agent info timestamp out of range"))?;
                let s = duration.num_seconds();
                let n = duration.clone().to_std().unwrap().subsec_nanos();
                // TODO FIXME
                #[allow(deprecated)]
                let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
                let duration = Duration::try_milliseconds(info.expires_at.as_micros() / 1000)
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
                writeln!(out, "URLs: {:?}", info.url)?;
                msg!("{}\n", out);
            }
        }
        AdminRequestCli::AgentMetaInfo(args) => {
            let info = client.agent_meta_info(args.url, args.dna).await?;
            let string_key_info = info
                .into_iter()
                .map(|(k, v)| (DnaHashB64::from(k).to_string(), v))
                .collect::<BTreeMap<String, BTreeMap<String, AgentMetaInfo>>>();

            let info_json = serde_json::to_string_pretty(&string_key_info)?;
            println!("{}", info_json);
        }
    }
    Ok(())
}

/// Calls [`AdminWebsocket::register_dna`] and registers the DNA.
async fn register_dna(client: &mut AdminWebsocket, args: RegisterDna) -> anyhow::Result<DnaHash> {
    let RegisterDna {
        network_seed,
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
        _ => unreachable!("Can't have hash and path for DNA source"),
    };
    let dna = RegisterDnaPayload {
        modifiers: DnaModifiersOpt {
            properties,
            network_seed,
        },
        source,
    };

    Ok(client.register_dna(dna).await?)
}

/// Constructs install payload with roles settings and calls
/// [`AdminWebsocket::install_app`] to install the provided app.
pub async fn install_app_bundle(
    client: &mut AdminWebsocket,
    args: InstallApp,
) -> anyhow::Result<AppInfo> {
    let InstallApp {
        app_id,
        agent_key,
        path,
        network_seed,
        roles_settings,
    } = args;

    let roles_settings = match roles_settings {
        Some(path) => {
            let yaml_string = std::fs::read_to_string(path)?;
            let roles_settings_yaml = serde_yaml::from_str::<RoleSettingsMapYaml>(&yaml_string)?;
            let mut roles_settings: RoleSettingsMap = HashMap::new();
            for (k, v) in roles_settings_yaml.into_iter() {
                roles_settings.insert(k, v.into());
            }
            Some(roles_settings)
        }
        None => None,
    };

    let payload = InstallAppPayload {
        installed_app_id: app_id.clone(),
        agent_key,
        source: AppBundleSource::Path(path),
        roles_settings,
        network_seed,
        ignore_genesis_failure: false,
    };

    let installed_app = client.install_app(payload).await?;

    match &installed_app.manifest {
        AppManifest::V0(manifest) => {
            if !manifest.allow_deferred_memproofs {
                client
                    .enable_app(installed_app.installed_app_id.clone())
                    .await?;
            }
        }
    }

    msg!("App installed with id {:?}.", app_id);

    Ok(installed_app)
}

/// Calls [`AdminWebsocket::add_agent_info`] and adds the list of agent info.
pub async fn add_agent_info(
    client: &mut AdminWebsocket,
    args: Vec<Arc<AgentInfoSigned>>,
) -> anyhow::Result<()> {
    let mut agent_infos = Vec::new();
    for info in args {
        agent_infos.push(info.encode()?);
    }
    Ok(client.add_agent_info(agent_infos).await?)
}

/// Calls [`AdminWebsocket::agent_info`] and pretty prints the agent info on this conductor.
async fn request_agent_info(
    client: &mut AdminWebsocket,
    args: ListAgents,
) -> anyhow::Result<Vec<Arc<AgentInfoSigned>>> {
    let resp = client.agent_info(args.dna).await?;
    let mut out = Vec::new();
    for info in resp {
        out.push(AgentInfoSigned::decode(
            &kitsune2_core::Ed25519Verifier,
            info.as_bytes(),
        )?);
    }

    Ok(out)
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
