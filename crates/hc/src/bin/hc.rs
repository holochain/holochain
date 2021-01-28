use hc::cmds::*;
use holochain_hc as hc;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// Holochain CLI - Helper for generating, running interacting with Holochain setups.
///
/// A setup is a directory containing the conductor config, databases and keystore.
/// Everything you need to quickly run your app in holochain or create complex setups.
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
    /// Generate a new holochain setup for later use.
    Generate {
        #[structopt(short, long, default_value = "1")]
        /// Number of conductors to create.
        num_conductors: usize,
        #[structopt(flatten)]
        gen: Create,
        #[structopt(short, long, value_delimiter = ",")]
        /// Generate a new setups and then run them.
        ///
        /// Optionally create an app interface.
        /// This allows you UI to talk to the conductor.
        /// For example `hc generate -r=0,9000,0`
        /// Or `hc generate -r` to run without attaching any app ports.
        run: Option<Vec<u16>>,
        /// List of dnas to run.
        /// Defaults to searching the current directory for
        /// a single `*.dna.gz` file.
        dnas: Vec<PathBuf>,
    },
    /// Run a conductor from existing setup.
    Run(Run),
    /// Call the conductor admin interface.
    Call(hc::calls::Call),
    /// [WIP unimplemented]: Run custom tasks using cargo task
    Task,
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
    /// Optionally create an app interface.
    /// This allows you UI to talk to the conductor.
    /// For example `hc run -p=0,9000,0`
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
                run_n(&ops.holochain_path, paths, ports, ops.force_admin_ports).await?;
            }
        }
        Op::Run(Run { ports, existing }) => {
            let paths = existing.load()?;
            if paths.is_empty() {
                return Ok(());
            }
            run_n(&ops.holochain_path, paths, ports, ops.force_admin_ports).await?;
        }
        // Op::Run(Run { ports, .. }) => {
        //     // Check if current directory has saved existing
        //     let existing = hc::save::load(std::env::current_dir()?)?;
        //     // If we have existing setups and we are not trying to generate and
        //     // we are not trying to load specific dnas then use existing.
        //     let paths = if !existing.is_empty() && gen.is_none() && dnas.is_empty() {
        //         existing
        //     } else {
        //         let create = gen.map(|g| g.into_inner()).unwrap_or_default();
        //         generate(&ops.holochain_path, dnas, num_conductors, create).await?
        //     };
        //     run_n(&ops.holochain_path, paths, ports, ops.force_admin_ports).await?;
        // }
        Op::Call(call) => hc::calls::call(&ops.holochain_path, call).await?,
        Op::Task => todo!("Running custom tasks is coming soon"),
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
