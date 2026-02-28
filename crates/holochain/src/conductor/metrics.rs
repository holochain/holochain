use opentelemetry::{global::meter, metrics::Histogram};

pub type PostCommitDurationMetric = Histogram<f64>;

pub fn create_post_commit_duration_metric() -> PostCommitDurationMetric {
    meter("hc.conductor")
        .f64_histogram("hc.conductor.post_commit.duration")
        .with_unit("s")
        .with_description("The time spent executing a post commit")
        .build()
}
