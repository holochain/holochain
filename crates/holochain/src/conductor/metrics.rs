use opentelemetry_api::{global::meter_with_version, metrics::*};

pub type P2pEventsMetric = Histogram<f64>;

pub fn create_p2p_event_duration_metric() -> P2pEventsMetric {
    meter_with_version(
        "hc.conductor",
        None::<&'static str>,
        None::<&'static str>,
        Some(vec![]),
    )
    .f64_histogram("hc.conductor.p2p_event.duration")
    .with_unit(Unit::new("s"))
    .with_description("The time spent processing a p2p event")
    .init()
}
