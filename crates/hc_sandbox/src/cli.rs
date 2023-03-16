//! Definitions of StructOpt options for use in the CLI

use crate::cmds::*;
use holochain_types::prelude::InstalledAppId;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

const DEFAULT_APP_ID: &str = "test-app";

#[derive(Debug, StructOpt)]
/// Helper for generating, running, and interacting with Holochain Conductor "sandboxes".
///
/// A sandbox is a directory containing a conductor config, databases, and keystore,
/// with a single Holochain app installed in the conductor:
/// Everything you need to quickly run your app in Holochain,
/// or create complex multi-conductor setups for testing.
pub struct HcSandbox {
    #[structopt(subcommand)]
    command: HcSandboxSubcommand,

    /// Instead of the normal "interactive" passphrase mode,
    /// collect the passphrase by reading stdin to the end.
    #[structopt(long)]
    piped: bool,

    /// Force the admin port that hc uses to talk to Holochain to a specific value.
    /// For example `hc -f=9000,9001 run`
    /// This must be set on each run or the port will change if it's in use.
    #[structopt(short, long, value_delimiter = ",")]
    force_admin_ports: Vec<u16>,

    /// Set the path to the holochain binary.
    #[structopt(short, long, env = "HC_HOLOCHAIN_PATH", default_value = "holochain")]
    holochain_path: PathBuf,
}

/// The list of subcommands for `hc sandbox`.
#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::InferSubcommands)]
pub enum HcSandboxSubcommand {
    /// Generate one or more new Holochain Conductor sandbox(es) for later use.
    ///
    /// A single app will be installed as part of this sandbox.
    Generate {
        #[structopt(short, long, default_value = DEFAULT_APP_ID)]
        /// ID for the installed app.
        /// This is just a string to identify the app by.
        app_id: InstalledAppId,

        /// (flattened)
        #[structopt(flatten)]
        create: Create,

        /// Automatically run the sandbox(es) that were created.
        /// This is effectively a combination of `hc sandbox generate` and `hc sandbox run`.
        /// You may optionally specify app interface ports to bind when running.
        /// This allows your UI to talk to the conductor.
        /// For example, `hc sandbox generate -r=0,9000,0` will create three app interfaces.
        /// Or, use `hc sandbox generate -r` to run without attaching any app interfaces.
        /// This follows the same structure as `hc sandbox run --ports`.
        #[structopt(short, long, value_delimiter = ",")]
        run: Option<Vec<u16>>,

        /// A hApp bundle to install.
        happ: Option<PathBuf>,
    },
    /// Run conductor(s) from existing sandbox(es).
    Run(Run),

    /// Make a call to a conductor's admin interface.
    Call(crate::calls::Call),

    /// List sandboxes found in `$(pwd)/.hc`.
    List {
        /// Show more verbose information.
        #[structopt(short, long, parse(from_occurrences))]
        verbose: usize,
    },

    /// Clean (completely remove) sandboxes that are listed in the `$(pwd)/.hc` file.
    Clean,

    /// Create a fresh sandbox with no apps installed.
    Create(Create),
}

/// Options for running a sandbox
#[derive(Debug, StructOpt)]
pub struct Run {
    /// Optionally specifies app interface ports to bind when running.
    /// This allows your UI to talk to the conductor.
    /// For example, `hc -p=0,9000,0` will create three app interfaces.
    /// Important: Interfaces are persistent. If you add an interface
    /// it will be there next time you run the conductor.
    #[structopt(short, long, value_delimiter = ",")]
    ports: Vec<u16>,

    /// (flattened)
    #[structopt(flatten)]
    existing: Existing,
}

impl HcSandbox {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        holochain_util::pw::pw_set_piped(self.piped);
        match self.command {
            HcSandboxSubcommand::Generate {
                app_id,
                create,
                run,
                happ,
            } => {
                let paths = generate(&self.holochain_path, happ, create, app_id).await?;
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
                    tokio::task::spawn(async move {
                        if let Err(e) =
                            run_n(&holochain_path, paths, ports, force_admin_ports).await
                        {
                            tracing::error!(failed_to_run = ?e);
                        }
                    });
                    tokio::signal::ctrl_c().await?;
                    crate::save::release_ports(std::env::current_dir()?).await?;
                }
            }
            HcSandboxSubcommand::Run(Run { ports, existing }) => {
                let paths = existing.load()?;
                if paths.is_empty() {
                    return Ok(());
                }
                let holochain_path = self.holochain_path.clone();
                let force_admin_ports = self.force_admin_ports.clone();
                tokio::task::spawn(async move {
                    if let Err(e) = run_n(&holochain_path, paths, ports, force_admin_ports).await {
                        tracing::error!(failed_to_run = ?e);
                    }
                });
                tokio::signal::ctrl_c().await?;
                crate::save::release_ports(std::env::current_dir()?).await?;
            }
            HcSandboxSubcommand::Call(call) => {
                crate::calls::call(&self.holochain_path, call).await?
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
            }) => {
                let mut paths = Vec::with_capacity(num_sandboxes);
                msg!(
                    "Creating {} conductor sandboxes with same settings",
                    num_sandboxes
                );
                for i in 0..num_sandboxes {
                    let path = crate::generate::generate(
                        network.clone().map(|n| n.into_inner().into()),
                        root.clone(),
                        directories.get(i).cloned(),
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

async fn run_n(
    holochain_path: &Path,
    paths: Vec<PathBuf>,
    app_ports: Vec<u16>,
    force_admin_ports: Vec<u16>,
) -> anyhow::Result<()> {
    let run_holochain = |holochain_path: PathBuf, path: PathBuf, ports, force_admin_port| async move {
        crate::run::run(&holochain_path, path, ports, force_admin_port).await?;
        Result::<_, anyhow::Error>::Ok(())
    };
    let mut force_admin_ports = force_admin_ports.into_iter();
    let mut app_ports = app_ports.into_iter();
    let jhs = paths
        .into_iter()
        .zip(std::iter::repeat_with(|| force_admin_ports.next()))
        .zip(std::iter::repeat_with(|| app_ports.next()))
        .map(|((path, force_admin_port), app_port)| {
            let f = run_holochain(
                holochain_path.to_path_buf(),
                path,
                app_port.map(|p| vec![p]).unwrap_or_default(),
                force_admin_port,
            );
            tokio::task::spawn(f)
        });
    futures::future::try_join_all(jhs).await?;
    Ok(())
}

async fn generate(
    holochain_path: &Path,
    happ: Option<PathBuf>,
    create: Create,
    app_id: InstalledAppId,
) -> anyhow::Result<Vec<PathBuf>> {
    let happ = crate::bundles::parse_happ(happ)?;
    let paths = crate::sandbox::default_n(holochain_path, create, happ, app_id).await?;
    crate::save::save(std::env::current_dir()?, paths.clone())?;
    Ok(paths)
}
