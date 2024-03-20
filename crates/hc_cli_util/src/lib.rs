use clap::{Parser, Subcommand};
use kitsune_p2p_dht::{arq::ArqSet, spacetime::Topology, Arq, ArqBounds, ArqStrat};
use kitsune_p2p_dht_arc::{DhtArc, DhtArcRange, DhtArcSet};

/// Utility functions for holochain debugging and diagnostics.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Util {
    /// The `hc util` subcommand to run.
    #[command(subcommand)]
    pub subcommand: UtilSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum UtilSubCommand {
    Arq {
        values: Vec<u32>,

        /// Calculate the approximate arq
        #[arg(long, short, group = "precision")]
        approx: bool,

        /// Only do exact calculations, no rounding or approximation
        #[arg(long, short, group = "precision")]
        exact: bool,
    },
}

impl Util {
    pub async fn run(self) {
        let topo = Topology::standard_epoch_full();
        let strat = ArqStrat::default();
        match self.subcommand {
            UtilSubCommand::Arq {
                values,
                approx,
                exact,
            } => {
                if values.len() == 2 {
                    let start = values[0];
                    let end = values[1];
                    let arc = DhtArc::from_bounds(start, end);
                    let arq = if approx {
                        Some(Arq::from_dht_arc_approximate(&topo, &strat, &arc))
                    } else if exact {
                        todo!()
                    } else {
                        panic!("Must set exactly one of --approx or --exact")
                    };
                    if let Some(arq) = arq {
                        println!("{:?}", arq);
                    } else {
                        println!("No can do.");
                    }
                } else if values.len() == 3 {
                    let power = values[0] as u8;
                    let start = values[1];
                    let count = values[2];

                    let arq = ArqBounds::new(power, start.into(), count.into());
                    let arc = arq.to_dht_arc_range(&topo);
                    println!("{:?}", arc);
                } else {
                    panic!("Usage: hc util arq START END  -or-  hc util arq POWER START COUNT")
                }
            }
        }
    }
}
