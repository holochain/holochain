use ha::scripts::*;
use holochain_admin as ha;
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
    force_admin_port: Option<u16>,
}

#[derive(Debug, StructOpt)]
enum Op {
    /// Create a fresh holochain setup and exit.
    Create(Create),
    /// Run holochain from existing or new setup.
    Run {
        #[structopt(subcommand)]
        /// Choose to run an existing setup or create an new one.
        run: Run,
        #[structopt(short, long)]
        /// Optionally create an app interface.
        port: Option<u16>,
        #[structopt(short, long)]
        /// Specify a secondary admin port so you can make admin requests while holochain is running.
        /// Set to 0 to let OS choose a free port.
        secondary_admin_port: Option<u16>,
    },
    Call(ha::calls::Call),
    Script(Script),
}

#[derive(Debug, StructOpt)]
enum Run {
    /// Create a new setup then run.
    New(Create),
    /// Run from an existing setup.
    Existing {
        #[structopt(short, long)]
        /// The path to the existing setup director.
        /// For example `/tmp/kAOXQlilEtJKlTM_W403b`
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::test_run().expect("Failed to start contextual logging");
    let ops = Ops::from_args();
    let run_holochain = |path: PathBuf, port, secondary_admin_port, force_admin_port| async move {
        if let Some(secondary_admin_port) = secondary_admin_port {
            let secondary_admin_port = if secondary_admin_port == 0 {
                None
            } else {
                Some(secondary_admin_port)
            };
            ha::add_secondary_admin_port(path.clone(), secondary_admin_port)?;
        }
        ha::run(path, port, force_admin_port).await?;
        Result::<_, anyhow::Error>::Ok(())
    };
    match ops.op {
        Op::Create(create) => {
            let path = default_with_network(create).await?;
            if let Some(port) = ops.force_admin_port {
                ha::force_admin_port(path.clone(), port)?;
            }
        }
        Op::Run {
            run: Run::Existing { path },
            port,
            secondary_admin_port,
        } => run_holochain(path, port, secondary_admin_port, ops.force_admin_port).await?,
        Op::Run {
            run: Run::New(create),
            port,
            secondary_admin_port,
        } => {
            let path = default_with_network(create).await?;
            if let Some(port) = ops.force_admin_port {
                ha::force_admin_port(path.clone(), port)?;
            }
            run_holochain(path, port, secondary_admin_port, ops.force_admin_port).await?;
        }
        Op::Call(call) => ha::calls::call(call).await?,
        Op::Script(script) => match script {
            Script::WithNetwork(create) => {
                default_with_network(create).await?;
            }
            Script::N {
                create,
                num_conductors,
            } => {
                default_n(create, num_conductors).await?;
            }
        },
    }

    Ok(())
}
