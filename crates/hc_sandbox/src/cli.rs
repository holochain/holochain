//! Definitions of Parser options for use in the CLI

use crate::cmds::*;
use clap::{ArgAction, Parser};
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_trace::Output;
use holochain_types::prelude::InstalledAppId;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;

const DEFAULT_APP_ID: &str = "test-app";

/// Helper for generating, running, and interacting with Holochain Conductor "sandboxes".
///
/// A sandbox is a directory containing a conductor config, databases, and keystore,
/// with a single Holochain app installed in the conductor:
/// Everything you need to quickly run your app in Holochain,
/// or create complex multi-conductor setups for testing.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct HcSandbox {
    #[command(subcommand)]
    subcommand: HcSandboxSubcommand,

    /// Instead of the normal "interactive" passphrase mode,
    /// collect the passphrase by reading stdin to the end.
    #[arg(long)]
    piped: bool,

    /// The log output option to use for Holochain.
    #[arg(long, default_value_t = Output::Log)]
    structured: Output,

    /// Force the admin port(s) that Holochain will use to a specific value.
    /// This option updates the conductor config file before starting Holochain
    /// and is only available with the `generate` and `run` commands.
    /// For example `hc sandbox -f=9000,9001 run`
    /// This must be set on each run or the port will change if it's in use.
    #[arg(short, long, value_delimiter = ',')]
    force_admin_ports: Vec<u16>,

    /// Set the path to the holochain binary.
    #[arg(
        short = 'H',
        long,
        env = "HC_HOLOCHAIN_PATH",
        default_value = "holochain"
    )]
    holochain_path: PathBuf,
}

/// The list of subcommands for `hc sandbox`.
#[derive(Debug, Parser)]
#[command(infer_subcommands = true)]
pub enum HcSandboxSubcommand {
    /// Generate one or more new Holochain Conductor sandbox(es) for later use.
    ///
    /// A single app will be installed as part of this sandbox.
    Generate {
        /// ID for the installed app.
        /// This is just a string to identify the app by.
        #[arg(short, long, default_value = DEFAULT_APP_ID)]
        app_id: InstalledAppId,

        /// (flattened)
        #[command(flatten)]
        create: Create,

        /// Automatically run the sandbox(es) that were created.
        /// This is effectively a combination of `hc sandbox generate` and `hc sandbox run`.
        /// You may optionally specify app interface ports to bind when running.
        /// This allows your UI to talk to the conductor.
        /// For example, `hc sandbox generate -r=0,9000,0` will create three app interfaces.
        /// Or, use `hc sandbox generate -r` to run without attaching any app interfaces.
        /// This follows the same structure as `hc sandbox run --ports`.
        #[arg(short, long, value_delimiter = ',')]
        run: Option<Vec<u16>>,

        /// A hApp bundle to install.
        happ: Option<PathBuf>,

        /// Network seed to use when installing the provided hApp.
        #[arg(long, short = 's')]
        network_seed: Option<String>,
    },

    /// Run conductor(s) from existing sandbox(es).
    Run(Run),

    /// Make a call to a conductor's admin interface.
    Call(crate::calls::Call),

    /// List sandboxes found in `$(pwd)/.hc`.
    List {
        /// Show more verbose information.
        #[arg(short, long, action = ArgAction::SetTrue)]
        verbose: bool,
    },

    /// Clean (completely remove) sandboxes that are listed in the `$(pwd)/.hc` file.
    Clean,

    /// Create a fresh sandbox with no apps installed.
    Create(Create),
}

/// Options for running a sandbox
#[derive(Debug, Parser)]
pub struct Run {
    /// Optionally specifies app interface ports to bind when running.
    /// This allows your UI to talk to the conductor.
    /// For example, `hc -p=0,9000,0` will create three app interfaces.
    /// Important: Interfaces are persistent. If you add an interface
    /// it will be there next time you run the conductor.
    #[arg(short, long, value_delimiter = ',')]
    ports: Vec<u16>,

    /// (flattened)
    #[command(flatten)]
    existing: Existing,
}

impl HcSandbox {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        holochain_util::pw::pw_set_piped(self.piped);
        match self.subcommand {
            HcSandboxSubcommand::Generate {
                app_id,
                create,
                run,
                happ,
                network_seed,
            } => {
                let paths = generate(
                    &self.holochain_path,
                    happ,
                    create,
                    app_id,
                    network_seed,
                    self.structured.clone(),
                )
                .await?;
                for (port, path) in self
                    .force_admin_ports
                    .clone()
                    .into_iter()
                    .zip(paths.clone().into_iter())
                {
                    crate::force_admin_port(path, port)?;
                }
                if let Some(ports) = run {
                    let holochain_path = self.holochain_path.clone();
                    let force_admin_ports = self.force_admin_ports.clone();
                    let structured = self.structured.clone();

                    let result = tokio::select! {
                        result = tokio::signal::ctrl_c() => result.map_err(anyhow::Error::from),
                        result = run_n(&holochain_path, paths, ports, force_admin_ports, structured) => result,
                    };
                    crate::save::release_ports(std::env::current_dir()?).await?;
                    return result;
                }
            }
            HcSandboxSubcommand::Run(Run { ports, existing }) => {
                let paths = existing.load()?;
                if paths.is_empty() {
                    tracing::warn!("no paths available, exiting.");
                    return Ok(());
                }
                let holochain_path = self.holochain_path.clone();
                let force_admin_ports = self.force_admin_ports.clone();

                let result = tokio::select! {
                    result = tokio::signal::ctrl_c() => result.map_err(anyhow::Error::from),
                    result = run_n(&holochain_path, paths.into_iter().map(ConfigRootPath::from).collect(), ports, force_admin_ports, self.structured) => result,
                };
                crate::save::release_ports(std::env::current_dir()?).await?;
                return result;
            }
            HcSandboxSubcommand::Call(call) => {
                crate::calls::call(
                    &self.holochain_path,
                    call,
                    self.force_admin_ports,
                    self.structured,
                )
                .await?
            }
            // HcSandboxSubcommand::Task => todo!("Running custom tasks is coming soon"),
            HcSandboxSubcommand::List { verbose } => {
                crate::save::list(std::env::current_dir()?, verbose)?
            }
            HcSandboxSubcommand::Clean => crate::save::clean(std::env::current_dir()?, Vec::new())?,
            HcSandboxSubcommand::Create(Create {
                num_sandboxes,
                network,
                root,
                directories,
                in_process_lair,
                no_dpki,
                #[cfg(feature = "chc")]
                chc_url,
            }) => {
                let mut paths = Vec::with_capacity(num_sandboxes);
                msg!(
                    "Creating {} conductor sandboxes with same settings",
                    num_sandboxes
                );
                for i in 0..num_sandboxes {
                    let network = Network::to_kitsune(&NetworkCmd::as_inner(&network)).await;
                    let path = crate::generate::generate(
                        network,
                        root.clone(),
                        directories.get(i).cloned(),
                        in_process_lair,
                        no_dpki,
                        #[cfg(feature = "chc")]
                        chc_url.clone(),
                    )?;
                    paths.push(path);
                }
                crate::save::save(std::env::current_dir()?, paths.clone())?;
                msg!("Created {:?}", paths);
            }
        }

        Ok(())
    }
}

/// Details about a conductor launched by the sandbox
#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchInfo {
    /// The admin port that was bound. This is not known when admin ports are not forced because the
    /// default is 0 so the system will choose a port.
    pub admin_port: u16,
    /// The app ports that were attached to the conductor.
    pub app_ports: Vec<u16>,
}

impl LaunchInfo {
    pub(crate) fn from_admin_port(admin_port: u16) -> Self {
        LaunchInfo {
            admin_port,
            app_ports: vec![],
        }
    }
}

/// Run a conductor for each path
pub async fn run_n(
    holochain_path: &Path,
    paths: Vec<ConfigRootPath>,
    app_ports: Vec<u16>,
    force_admin_ports: Vec<u16>,
    structured: Output,
) -> anyhow::Result<()> {
    let run_holochain = |holochain_path: PathBuf,
                         path: ConfigRootPath,
                         index: usize,
                         ports,
                         force_admin_port,
                         structured| async move {
        crate::run::run(
            &holochain_path,
            path,
            index,
            ports,
            force_admin_port,
            structured,
        )
        .await?;
        Result::<_, anyhow::Error>::Ok(())
    };
    let mut force_admin_ports = force_admin_ports.into_iter();
    let mut app_ports = app_ports.into_iter();
    let jhs = paths
        .into_iter()
        .enumerate()
        .zip(std::iter::repeat_with(|| force_admin_ports.next()))
        .zip(std::iter::repeat_with(|| app_ports.next()))
        .map(|(((index, path), force_admin_port), app_port)| {
            let f = run_holochain(
                holochain_path.to_path_buf(),
                path,
                index,
                app_port.map(|p| vec![p]).unwrap_or_default(),
                force_admin_port,
                structured.clone(),
            );
            tokio::task::spawn(f)
        });
    futures::future::try_join_all(jhs).await?;
    Ok(())
}

/// Perform the `generate` subcommand
pub async fn generate(
    holochain_path: &Path,
    happ: Option<PathBuf>,
    create: Create,
    app_id: InstalledAppId,
    network_seed: Option<String>,
    structured: Output,
) -> anyhow::Result<Vec<ConfigRootPath>> {
    let happ = crate::bundles::parse_happ(happ)?;
    let paths = crate::sandbox::default_n(
        holochain_path,
        create,
        happ,
        app_id,
        network_seed,
        structured,
    )
    .await?;
    crate::save::save(std::env::current_dir()?, paths.clone())?;
    Ok(paths)
}
