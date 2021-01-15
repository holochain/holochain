use hc::scripts::*;
use holochain_hc as hc;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
/// Holochain Admin - Helper for setting up holochain and making admin requests.
struct Ops {
    #[structopt(subcommand)]
    op: Op,
    /// Force the admin port to a specific value.
    /// Useful if you are setting this config
    /// up for use elsewhere (see also secondary_admin_port).
    #[structopt(short, long)]
    force_admin_ports: Vec<u16>,
    // TODO: Add holochain path as a parameter that
    // falls back to an env var then to `holochain`
    // on the path.
}

#[derive(Debug, StructOpt)]
enum Op {
    /// Generate a fresh holochain setup.
    Gen {
        #[structopt(flatten)]
        gen: Create,
        #[structopt(flatten)]
        source: Source,
    },
    /// Run holochain from existing or new setup.
    Run(Run),
    /// Call any of holochain admin interfaces
    Call(hc::calls::Call),
    /// WIP
    RunTask(Script),
}

#[derive(Debug, StructOpt)]
struct Run {
    #[structopt(short, long)]
    /// Optionally create an app interface.
    ports: Vec<u16>,
    #[structopt(short, long)]
    /// Specify a secondary admin port so you can make admin requests while holochain is running.
    /// Set to 0 to let OS choose a free port.
    secondary_admin_ports: Vec<u16>,
    /// Generate a new setup and then run.
    #[structopt(subcommand)]
    gen: Option<Gen>,
    #[structopt(short, long, conflicts_with_all = &["dnas", "gen"])]
    /// The path to the existing setup directory.
    /// For example `/tmp/kAOXQlilEtJKlTM_W403b`
    existing: Vec<PathBuf>,
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
    observability::test_run().expect("Failed to start contextual logging");
    let ops = Ops::from_args();
    match ops.op {
        Op::Gen {
            gen,
            source: Source {
                dnas,
                num_conductors,
            },
        } => {
            generate(dnas, num_conductors, gen).await?;
            // if let Some(_port) = ops.force_admin_port {
            //     // hc::force_admin_port(path[0].clone(), port)?;
            //     todo!("Force admin port is not implemented yet")
            // }
        }
        Op::Run(Run {
            ports,
            secondary_admin_ports,
            existing,
            ..
        }) if !existing.is_empty() => {
            // if let Some(port) = ops.force_admin_port {
            //     hc::force_admin_port(path.clone(), port)?;
            // }
            run_n(
                existing,
                ports,
                ops.force_admin_ports,
                secondary_admin_ports,
            )
            .await?;
        }
        Op::Run(Run {
            gen,
            ports,
            secondary_admin_ports,
            source: Source {
                dnas,
                num_conductors,
            },
            ..
        }) => {
            // Check if current directory has saved existing
            let existing = hc::load(std::env::current_dir()?)?;
            // If we have existing and we are not trying to generate and
            // we are not trying to load specific dnas then use existing.
            let paths = if !existing.is_empty() && gen.is_none() && dnas.is_empty() {
                existing
            } else {
                let create = gen.map(|g| g.into_inner()).unwrap_or_default();
                generate(dnas, num_conductors, create).await?
            };
            // TODO: Number of forced admin ports must be <= number of conductors
            // if let Some(_port) = ops.force_admin_port {
            //     // hc::force_admin_port(path[0].clone(), port)?;
            //     todo!("Force admin port is not implemented yet")
            // }
            run_n(paths, ports, ops.force_admin_ports, secondary_admin_ports).await?;
        }
        Op::Call(call) => hc::calls::call(call).await?,
        Op::RunTask(_) => (),
        // Op::RunTask(script) => match script {
        //     Script::WithNetwork(create) => {
        //         default_with_network(create).await?;
        //     }
        //     Script::N {
        //         create,
        //         num_conductors,
        //     } => {
        //         default_n(create, num_conductors).await?;
        //     }
        // },
    }

    Ok(())
}

async fn run_n(
    paths: Vec<PathBuf>,
    ports: Vec<u16>,
    force_admin_ports: Vec<u16>,
    secondary_admin_ports: Vec<u16>,
) -> anyhow::Result<()> {
    let run_holochain = |path: PathBuf, ports, secondary_admin_port, force_admin_port| async move {
        if let Some(secondary_admin_port) = secondary_admin_port {
            let secondary_admin_port = if secondary_admin_port == 0 {
                None
            } else {
                Some(secondary_admin_port)
            };
            hc::add_secondary_admin_port(path.clone(), secondary_admin_port)?;
        }
        hc::run(path, ports, force_admin_port).await?;
        Result::<_, anyhow::Error>::Ok(())
    };
    let mut force_admin_ports = force_admin_ports.into_iter();
    let mut secondary_admin_ports = secondary_admin_ports.into_iter();
    let jhs = paths
        .into_iter()
        .zip(std::iter::repeat_with(|| force_admin_ports.next()))
        .zip(std::iter::repeat_with(|| secondary_admin_ports.next()))
        .map(|((path, force_admin_port), secondary_admin_port)| {
            let f = run_holochain(path, ports.clone(), secondary_admin_port, force_admin_port);
            tokio::task::spawn(f)
        });
    futures::future::try_join_all(jhs).await?;
    Ok(())
}

async fn generate(
    dnas: Vec<PathBuf>,
    num_conductors: usize,
    create: Create,
) -> anyhow::Result<Vec<PathBuf>> {
    let dnas = hc::app::parse_dnas(dnas)?;
    let paths = default_n(num_conductors, create, dnas).await?;
    hc::save(std::env::current_dir()?, paths.clone())?;
    Ok(paths)
}
