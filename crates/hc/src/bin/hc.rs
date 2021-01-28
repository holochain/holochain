use hc::cmds::*;
use holochain_hc as hc;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// Holochain CLI - Helper for generating, running, and interacting with Holochain Conductor "setups".
///
/// A setup is a directory containing a conductor config, databases, and keystore,
/// with a single Holochain app installed in the conductor:
/// Everything you need to quickly run your app in holochain,
/// or create complex multi-conductor setups for testing.
struct Ops {
    #[structopt(subcommand)]
    op: Op,
    /// Force the admin port that hc uses to talk to holochain to a specific value.
    /// For example `hc -f=9000,9001 run`
    /// This must be set on each run or the port will change if it's in use.
    #[structopt(short, long, value_delimiter = ",")]
    force_admin_ports: Vec<u16>,
    /// Set the path to the holochain binary.
    #[structopt(short, long, env = "HC_HOLOCHAIN_PATH", default_value = "holochain")]
    holochain_path: PathBuf,
}

#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::InferSubcommands)]
enum Op {
    /// Generate one or more new Holochain Conductor setup(s) for later use.
    ///
    /// A single app will be installed as part of this setup.
    /// See the help for the `<dnas>` argument below to learn how to define the app to be installed.
    Generate {
        #[structopt(short, long, default_value = "1")]
        /// Number of conductor setups to create.
        num_conductors: usize,
        #[structopt(flatten)]
        gen: Create,
        #[structopt(short, long, value_delimiter = ",")]
        /// Automatically run the setup(s) that were created.
        /// This is effectively a combination of `hc generate` and `hc run`
        ///
        /// You may optionally specify app interface ports to bind when running.
        /// This allows your UI to talk to the conductor.
        ///
        /// For example, `hc generate -r=0,9000,0` will create three app interfaces.
        /// Or, use `hc generate -r` to run without attaching any app interfaces.
        ///
        /// This follows the same structure as `hc run --ports`
        run: Option<Vec<u16>>,
        /// List of DNAs to use when installing the App for this setup.
        /// Defaults to searching the current directory for a single `*.dna.gz` file.
        dnas: Vec<PathBuf>,
    },
    /// Run conductor(s) from existing setup(s).
    Run(Run),
    /// Make a call to a conductor's admin interface.
    Call(hc::calls::Call),
    // /// [WIP unimplemented]: Run custom tasks using cargo task
    // Task,
    /// List setups found in `$(pwd)/.hc`.
    List {
        /// Show more verbose information.
        #[structopt(short, long, parse(from_occurrences))]
        verbose: usize,
    },
    /// Clean (completely remove) setups that are listed in the `$(pwd)/.hc` file.
    Clean,
}

#[derive(Debug, StructOpt)]
struct Run {
    #[structopt(short, long, value_delimiter = ",")]
    /// Optionally specifies app interface ports to bind when running.
    /// This allows your UI to talk to the conductor.
    /// For example, `hc -p=0,9000,0` will create three app interfaces.
    ports: Vec<u16>,
    #[structopt(flatten)]
    existing: Existing,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var_os("RUST_LOG").is_some() {
        observability::init_fmt(observability::Output::Log).ok();
    }
    let ops = Ops::from_args();
    match ops.op {
        Op::Generate {
            gen,
            run,
            num_conductors,
            dnas,
        } => {
            let paths = generate(&ops.holochain_path, dnas, num_conductors, gen).await?;
            for (port, path) in ops
                .force_admin_ports
                .clone()
                .into_iter()
                .zip(paths.clone().into_iter())
            {
                hc::force_admin_port(path, port)?;
            }
            if let Some(ports) = run {
                let holochain_path = ops.holochain_path.clone();
                let force_admin_ports = ops.force_admin_ports.clone();
                tokio::task::spawn(async move {
                    if let Err(e) = run_n(&holochain_path, paths, ports, force_admin_ports).await {
                        tracing::error!(failed_to_run = ?e);
                    }
                });
                tokio::signal::ctrl_c().await?;
                hc::save::release_ports(std::env::current_dir()?).await?;
            }
        }
        Op::Run(Run { ports, existing }) => {
            let paths = existing.load()?;
            if paths.is_empty() {
                return Ok(());
            }
            let holochain_path = ops.holochain_path.clone();
            let force_admin_ports = ops.force_admin_ports.clone();
            tokio::task::spawn(async move {
                if let Err(e) = run_n(&holochain_path, paths, ports, force_admin_ports).await {
                    tracing::error!(failed_to_run = ?e);
                }
            });
            tokio::signal::ctrl_c().await?;
            hc::save::release_ports(std::env::current_dir()?).await?;
        }
        Op::Call(call) => hc::calls::call(&ops.holochain_path, call).await?,
        // Op::Task => todo!("Running custom tasks is coming soon"),
        Op::List { verbose } => hc::save::list(std::env::current_dir()?, verbose)?,
        Op::Clean => hc::save::clean(std::env::current_dir()?, Vec::new())?,
    }
    Ok(())
}

async fn run_n(
    holochain_path: &Path,
    paths: Vec<PathBuf>,
    app_ports: Vec<u16>,
    force_admin_ports: Vec<u16>,
) -> anyhow::Result<()> {
    let run_holochain = |holochain_path: PathBuf, path: PathBuf, ports, force_admin_port| async move {
        hc::run::run(&holochain_path, path, ports, force_admin_port).await?;
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
    dnas: Vec<PathBuf>,
    num_conductors: usize,
    create: Create,
) -> anyhow::Result<Vec<PathBuf>> {
    let dnas = hc::dna::parse_dnas(dnas)?;
    let paths = hc::setups::default_n(holochain_path, num_conductors, create, dnas).await?;
    hc::save::save(std::env::current_dir()?, paths.clone())?;
    Ok(paths)
}
