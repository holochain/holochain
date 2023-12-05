use holochain_trace::metrics;
use std::error::Error;
use tracing::*;

metrics!(MyMetric, CounterA, CounterB);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    holochain_trace::test_run().ok();
    holochain_trace::metrics::init();
    let span = debug_span!("span a");
    let _g = span.enter();

    MyMetric::count(MyMetric::CounterA, 30);
    MyMetric::count(MyMetric::CounterA, 40);
    MyMetric::count(MyMetric::CounterB, 40);
    MyMetric::count(MyMetric::CounterB, 40u64);

    MyMetric::count_filter(MyMetric::CounterA, 10, "my_filter");
    MyMetric::count_filter(MyMetric::CounterA, 10, "my_other_filter");

    MyMetric::print();
    let mut td = std::env::temp_dir();
    td.push("metrics_csv");
    std::fs::create_dir(&td).ok();
    td.push("metrics.csv");
    MyMetric::save_csv(&td);

    Ok(())
}
