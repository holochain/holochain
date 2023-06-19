use opentelemetry::{global, metrics::ObservableGauge, Context, Key, KeyValue, Value};

/// Record the number of open connections on a websocket server
pub struct WebsocketConnectionsMetric {
    attributes: Vec<KeyValue>,
    gauge: ObservableGauge<u64>,
}

impl WebsocketConnectionsMetric {
    /// Create a new metric handle with the port the websocket is listening on.
    pub fn new(listen_port: u16) -> Self {
        let meter = global::meter("holochain.ws.connections");
        let counter = meter.u64_observable_gauge("conn_count").init();

        WebsocketConnectionsMetric {
            attributes: vec![KeyValue {
                key: Key::from_static_str("listen_port"),
                value: Value::I64(listen_port as i64),
            }],
            gauge: counter,
        }
    }

    /// Record the current connection count.
    pub fn record_current(&self, connection_count: u64) {
        self.gauge.observe(connection_count, &self.attributes);
    }
}
