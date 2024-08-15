//! This is just the beginnings of a CLI tool for generating hc_sleuth reports.
//! It's nowhere near useful, so it's not even built yet.

use std::{path::PathBuf, str::FromStr};

use hc_sleuth::{aitia::Fact, Fact as HcFact};
use holochain_types::prelude::*;
use structopt::StructOpt;

fn main() {
    let opt = HcSleuth::from_args();

    match opt {
        HcSleuth::ShowGraph => {
            hc_sleuth::report(
                HcFact::Integrated {
                    by: "".into(),
                    op: DhtOpHash::from_raw_32(vec![0; 32]),
                },
                &Default::default(),
            );
        }
        HcSleuth::Report { hash, log_paths } => {
            let mut ctx = hc_sleuth::Context::default();
            for path in log_paths {
                let file = std::fs::File::open(path).unwrap();
                let reader = std::io::BufReader::new(file);
                ctx.apply_log(reader);
            }

            let ops = match (
                ActionHash::try_from(hash.clone()),
                DhtOpHash::try_from(hash.clone()),
            ) {
                (Ok(hash), Err(_)) => ctx.ops_from_action(&hash).unwrap(),
                (Err(_), Ok(hash)) => {
                    maplit::hashset![hash]
                }
                (Err(_), Err(_)) => {
                    eprintln!("Invalid hash: {}", hash);
                    return;
                }
                (Ok(_), Ok(_)) => {
                    unreachable!("Can't parse a hash as both an ActionHash and a DhtOpHash")
                }
            };

            let events: Vec<_> = ctx
                .events
                .iter()
                .filter(|(_, f)| f.op().map(|op| ops.contains(&op)).unwrap_or(false))
                // .filter(|(_, f)| {
                //     matches!(
                //         **f,
                //         HcFact::ReceivedHash { .. } | HcFact::SentHash { .. } | HcFact::Fetched { .. }
                //     )
                // })
                .collect();

            if events.is_empty() {
                println!("No filtered events found for hash {}", hash);
            } else {
                for (ts, fact) in events {
                    println!("{}: {}", ts, fact.explain(&ctx));
                }
            }
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "hc_sleuth",
    about = "Examine the causal relationships between events in Holochain"
)]
pub enum HcSleuth {
    ShowGraph,
    Report {
        #[structopt(
            help = "The base-64 (prefix \"uhC\") ActionHash or DhtOpHash to check for integration"
        )]
        hash: String,

        #[structopt(
            help = "Paths to the log file(s) which contain aitia-enabled logs with hc-sleuth events"
        )]
        log_paths: Vec<PathBuf>,
    },
    //
    // Query {
    //     #[structopt(
    //         short = "h",
    //         long,
    //         help = "The action or entry hash to check for integration"
    //     )]
    //     op_hash: TargetHash,
    //     #[structopt(
    //         short,
    //         long,
    //         help = "The node ID which integrated (check the `tracing_scope` setting of your conductor config for this value)"
    //     )]
    //     node: String,
    //     log_paths: Vec<PathBuf>,
    // },
}

#[derive(Debug, derive_more::Deref)]
pub struct TargetHash(AnyDhtHash);

impl FromStr for TargetHash {
    type Err = HoloHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hash = AnyDhtHashB64::from_b64_str(s)?;
        Ok(Self(hash.into()))
    }
}
