use hc::cmds::*;
use holochain_hc as hc;
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// Holochain CLI - Helper for generating, running interacting with Holochain setups.
struct Ops {
    #[structopt(subcommand)]
    op: Op,
    /// Force the admin port to a specific value.
    /// Useful if you are setting this config
    /// up for use elsewhere (see also secondary_admin_port).
    /// For example `hc -f=9000,9001`
    #[structopt(short, long, value_delimiter = ",")]
    force_admin_ports: Vec<u16>,
    /// Set the path to the holochain binary.
    #[structopt(short, long, env = "HC_HOLOCHAIN_PATH", default_value = "holochain")]
    holochain_path: PathBuf,
}

#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::InferSubcommands)]
enum Op {
    /// Generate a fresh holochain setup.
    Generate {
        #[structopt(flatten)]
        gen: Create,
        #[structopt(flatten)]
        source: Source,
    },
    /// Run a conductor from existing or new setup.
    Run(Run),
    /// Call conductor admin interfaces.
    Call(hc::calls::Call),
    /// [WIP unimplemented]: Run custom tasks using cargo task
    Task,
    /// List setups found in `$(pwd)/.hc`.
    /// Note that indices will change if setups are removed.
    List {
        /// Show more verbose information.
        #[structopt(short, long, parse(from_occurrences))]
        verbose: usize,
    },
    /// Clean setups that are listed in the current directory
    /// inside the `.hc` file.
    Clean {
        /// Setups to clean.
        /// If blank all will be cleaned.
        setups: Vec<usize>,
    },
}

#[derive(Debug, StructOpt)]
struct Run {
    #[structopt(short, long, value_delimiter = ",")]
    /// Optionally create an app interface.
    /// This allows you UI to talk to the conductor.
    /// For example `hc run -p=0,9000,0`
    ports: Vec<u16>,
    #[structopt(short, long, value_delimiter = ",")]
    /// Specify a secondary admin port so you can make admin requests while holochain is running.
    /// For example: `hc run -s=0,9000,1`
    /// Set to 0 to let OS choose a free port.
    /// Defaults to all 0.
    /// Set `disable-secondary` to disable this.
    secondary_admin_ports: Vec<u16>,
    #[structopt(short, long)]
    disable_secondary: bool,
    /// Generate a new setup and then run.
    #[structopt(subcommand)]
    gen: Option<Gen>,
    #[structopt(flatten)]
    existing: Existing,
    #[structopt(flatten)]
    source: Source,
}

#[derive(Debug, StructOpt)]
enum Gen {
    Gen(Create),
}

#[derive(Debug, StructOpt)]
struct Source {
    #[structopt(short, long, default_value = "1")]
    /// Number of conductors to create or run.
    /// Must be <= to number of existing setups.
    /// Defaults to 1.
    num_conductors: usize,
    /// List of dnas to run.
    /// Defaults to the current directory
    dnas: Vec<PathBuf>,
}

impl Gen {
    pub fn into_inner(self) -> Create {
        match self {
            Gen::Gen(n) => n,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::test_run().ok();
    let ops = Ops::from_args();
    match ops.op {
        Op::Generate {
            gen,
            source: Source {
                dnas,
                num_conductors,
            },
        } => {
            let paths = generate(&ops.holochain_path, dnas, num_conductors, gen).await?;
            for (port, path) in ops.force_admin_ports.into_iter().zip(paths.into_iter()) {
                hc::force_admin_port(path, port)?;
            }
        }
        Op::Run(Run {
            ports,
            secondary_admin_ports,
            disable_secondary,
            existing,
            ..
        }) if !existing.is_empty() => {
            let paths = existing.load()?;
            run_n(
                &ops.holochain_path,
                paths,
                ports,
                ops.force_admin_ports,
                secondary_admin_ports,
                disable_secondary,
            )
            .await?;
        }
        Op::Run(Run {
            gen,
            ports,
            secondary_admin_ports,
            disable_secondary,
            source: Source {
                dnas,
                num_conductors,
            },
            ..
        }) => {
            // Check if current directory has saved existing
            let existing = hc::save::load(std::env::current_dir()?)?;
            // If we have existing setups and we are not trying to generate and
            // we are not trying to load specific dnas then use existing.
            let paths = if !existing.is_empty() && gen.is_none() && dnas.is_empty() {
                existing
            } else {
                let create = gen.map(|g| g.into_inner()).unwrap_or_default();
                generate(&ops.holochain_path, dnas, num_conductors, create).await?
            };
            run_n(
                &ops.holochain_path,
                paths,
                ports,
                ops.force_admin_ports,
                secondary_admin_ports,
                disable_secondary,
            )
            .await?;
        }
        Op::Call(call) => hc::calls::call(&ops.holochain_path, call).await?,
        Op::Task => todo!("Running custom tasks is coming soon"),
        Op::List { verbose } => hc::save::list(std::env::current_dir()?, verbose)?,
        Op::Clean { setups } => hc::save::clean(std::env::current_dir()?, setups)?,
    }

    Ok(())
}

async fn run_n(
    holochain_path: &Path,
    paths: Vec<PathBuf>,
    ports: Vec<u16>,
    force_admin_ports: Vec<u16>,
    mut secondary_admin_ports: Vec<u16>,
    disable_secondary: bool,
) -> anyhow::Result<()> {
    let run_holochain = |holochain_path: PathBuf,
                         path: PathBuf,
                         ports,
                         secondary_admin_port,
                         force_admin_port| async move {
        if let Some(secondary_admin_port) = secondary_admin_port {
            let secondary_admin_port = if secondary_admin_port == 0 {
                None
            } else {
                Some(secondary_admin_port)
            };
            hc::add_secondary_admin_port(path.clone(), secondary_admin_port)?;
        }
        hc::run::run(&holochain_path, path, ports, force_admin_port).await?;
        Result::<_, anyhow::Error>::Ok(())
    };
    if !disable_secondary && secondary_admin_ports.is_empty() {
        secondary_admin_ports = vec![0; paths.len()];
    }
    let mut force_admin_ports = force_admin_ports.into_iter();
    let mut secondary_admin_ports = secondary_admin_ports.into_iter();
    let jhs = paths
        .into_iter()
        .zip(std::iter::repeat_with(|| force_admin_ports.next()))
        .zip(std::iter::repeat_with(|| secondary_admin_ports.next()))
        .map(|((path, force_admin_port), secondary_admin_port)| {
            let f = run_holochain(
                holochain_path.to_path_buf(),
                path,
                ports.clone(),
                secondary_admin_port,
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
