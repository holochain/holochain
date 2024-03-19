use clap::{Parser, Subcommand};
use kitsune_p2p_dht::{arq::ArqSet, spacetime::Topology, ArqBounds, ArqStrat};
use kitsune_p2p_dht_arc::{DhtArcRange, DhtArcSet};

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
    Arq { values: Vec<u32> },
}

impl Util {
    pub async fn run(self) {
        let topo = Topology::standard_epoch_full();
        let strat = ArqStrat::default();
        match self.subcommand {
            UtilSubCommand::Arq { values } => {
                if values.len() == 2 {
                    let start = values[0];
                    let end = values[1];
                    let arc = DhtArcRange::from_bounds(start, end);
                    let arcset = DhtArcSet::from_interval(arc);
                    if let Some(arqset) = ArqSet::from_dht_arc_set_exact(&topo, &strat, &arcset) {
                        println!("{:?}", arqset);
                    } else {
                        println!("No can do.");
                    }
                } else if values.len() == 3 {
                    let power = values[0];
                    let start = values[1];
                    let count = values[2];
                } else {
                    panic!("Usage: hc util arq START END  -or-  hc util arq POWER START COUNT")
                }
            }
        }
    }
}
