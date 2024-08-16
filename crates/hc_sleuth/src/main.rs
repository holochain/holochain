//! This is just the beginnings of a CLI tool for generating hc_sleuth reports.
//! It's nowhere near useful, so it's not even built yet.

use std::{borrow::Cow, path::PathBuf, str::FromStr};

use hc_sleuth::aitia::Fact;
use holochain_types::prelude::*;
use regex::Regex;
use structopt::StructOpt;

fn build_context(logs: Vec<PathBuf>) -> hc_sleuth::Context {
    let mut ctx = hc_sleuth::Context::default();
    for path in logs {
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);
        ctx.apply_log(reader);
    }
    ctx
}

fn shortening(s: &str, n: usize) -> Cow<'_, str> {
    Regex::new(r"uhC(.)k(.{48})")
        .unwrap()
        .replace_all(s, |caps: &regex::Captures| {
            let ch = &caps[1];
            let pre = match ch {
                "Q" => " DhtOp",
                "k" => "Action",
                "E" => " Entry",
                "A" => " Agent",
                _ => unreachable!("Unknown hash prefix hC{ch}k"),
            };
            let hash = &caps[2][48 - n..];
            format!("{pre}{{{hash}}}")
        })
}

fn main() {
    holochain_trace::test_run();
    let opt = HcSleuth::from_args();

    match opt {
        HcSleuth::Events {
            hash,
            log_paths,
            shorten,
            encoded,
        } => {
            let ctx = build_context(log_paths);

            let events: Vec<_> = if let Some(hash) = hash.as_ref() {
                let ops = match InputHash::from_str(hash).expect("Invalid hash") {
                    InputHash::Action(hash) => ctx.ops_from_action(&hash).unwrap(),
                    InputHash::DhtOp(hash) => {
                        maplit::hashset![hash]
                    }
                };

                ctx.events
                    .iter()
                    .filter(|(_, f, _)| f.op().map(|op| ops.contains(op)).unwrap_or(false))
                    .collect()
            } else {
                ctx.events.iter().collect()
            };

            if events.is_empty() {
                if let Some(hash) = hash {
                    println!("No events found for hash {}", hash);
                } else {
                    println!("No events found");
                }
            } else {
                for (ts, fact, raw) in events {
                    let show = fact.explain(&ctx);
                    let show = shortening(&show, shorten);
                    if encoded {
                        println!("{ts}: {show}   {raw}");
                    } else {
                        println!("{ts}: {show}");
                    }
                }
            }
        } //
          //
          // HcSleuth::ShowGraph {
          //     event,
          //     log_paths,
          //     shorten,
          // } => {
          //     let ctx = build_context(log_paths);
          //     let event = Event::decode(&event);
          //     if let Some(report) = aitia::simple_report(&event.fact.traverse(&ctx)) {
          //         println!("{}", shortening(&report, shorten));
          //     } else {
          //         // No report means success
          //     }
          // }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "hc_sleuth",
    about = "Examine the causal relationships between events in Holochain"
)]
pub enum HcSleuth {
    Events {
        #[structopt(
            help = "The base-64 ActionHash or DhtOpHash (prefix \"uhCkk\" or \"uhCQk\") to check for integration"
        )]
        hash: Option<String>,

        #[structopt(
            short,
            long = "log",
            help = "Paths to the log file(s) which contain aitia-enabled logs with hc-sleuth events"
        )]
        log_paths: Vec<PathBuf>,

        #[structopt(
            short,
            long,
            default_value = "4",
            help = "Shorten hashes in output to the last N base64 characters"
        )]
        shorten: usize,

        #[structopt(
            short,
            long,
            help = "Include the base64 event encoding in the output, useful for input to `hc-sleuth show-graph`"
        )]
        encoded: bool,
    },
    //
    //
    // ShowGraph {
    //     #[structopt(help = "The base-64 encoded aitia Event to show the graph for")]
    //     event: String,

    //     #[structopt(
    //         short,
    //         long = "log",
    //         help = "Paths to the log file(s) which contain aitia-enabled logs with hc-sleuth events"
    //     )]
    //     log_paths: Vec<PathBuf>,

    //     #[structopt(
    //         short,
    //         long,
    //         default_value = "4",
    //         help = "Shorten hashes in output to the last N base64 characters"
    //     )]
    //     shorten: usize,
    // },
}

pub enum InputHash {
    Action(ActionHash),
    DhtOp(DhtOpHash),
}

impl FromStr for InputHash {
    type Err = HoloHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match (
            ActionHash::try_from(s.to_string()),
            DhtOpHash::try_from(s.to_string()),
        ) {
            (Ok(hash), Err(_)) => Ok(InputHash::Action(hash)),
            (Err(_), Ok(hash)) => Ok(InputHash::DhtOp(hash)),
            (Err(_), Err(_)) => Err(HoloHashError::BadBase64),
            (Ok(_), Ok(_)) => {
                unreachable!("Can't parse a hash as both an ActionHash and a DhtOpHash")
            }
        }
    }
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
