use opentelemetry_api::{global::meter_with_version, metrics::*};

pub type PostCommitDurationMetric = Histogram<f64>;

pub fn create_post_commit_duration_metric() -> PostCommitDurationMetric {
    meter_with_version(
        "hc.conductor",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.conductor.post_commit.duration")
    .with_unit(Unit::new("s"))
    .with_description("The time spent executing a post commit")
    .init()
}
