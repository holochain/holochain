use sx_types::observability::{self, Output};
use structopt::StructOpt;
use tokio::runtime::Runtime;
use tracing::*;

const RUN_LEN: usize = 1;

#[derive(Debug, StructOpt)]
#[structopt(name = "holochain", about = "The holochain conductor.")]
struct Opt {
    #[structopt(
        long,
        help = "Outputs structured json from logging:
    - None: No logging at all (fastest)
    - Log: Output logs to stdout with spans (human readable)
    - Compact: Same as Log but with less information
    - Json: Output logs as structured json (machine readable)
    ",
        default_value = "Log"
    )]
    structured: Output,
    #[structopt(long)]
    trace_id: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();
    observability::init_fmt(opt.structured.clone()).expect("Failed to start contextual logging");
    let mut rt = Runtime::new()?;
    rt.block_on(run(opt));
    Ok(())
}

async fn run(opt: Opt) {
    for _ in 0..RUN_LEN {
        let s = trace_span!("loop");
        let _g = s.enter();
        if let Some(trace_id) = opt.trace_id.clone() {
            trace!(trace_id = %trace_id)
        }
        trace!("Test");
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
    }
}
