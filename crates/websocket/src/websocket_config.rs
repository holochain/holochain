//! defines a builder-style config struct for setting up websockets

/// A builder-style config struct for setting up websockets.
#[derive(Debug)]
pub struct WebsocketConfig {
    /// Scheme to use for urls - e.g. "ws" or "wss". [default = "ws"]
    pub scheme: &'static str,

    /// Time after which the lib will stop tracking individual request ids.
    /// [default = 30_000]
    pub default_request_timeout_ms: u64,

    /// We will treat the socket as disconnected if we receive no messages
    /// in this timeframe.
    /// If Some(_), this setting will also trigger automatic Ping messages
    /// at half this timeframe.
    /// [default = 30_000]
    pub latency_disconnect_ms: Option<u64>,

    /// How many items are allowed in the outgoing queue. [default = 10]
    pub max_send_queue: usize,

    /// Maximum total message size of a websocket message. [default = 64M]
    pub max_message_size: usize,

    /// Maximum websocket frame size. [default = 16M]
    pub max_frame_size: usize,
}

impl Default for WebsocketConfig {
    fn default() -> Self {
        Self {
            scheme: "ws",
            default_request_timeout_ms: 30_000,
            latency_disconnect_ms: Some(30_000),
            max_send_queue: 10,
            max_message_size: 64 << 20,
            max_frame_size: 16 << 20,
        }
    }
}

impl WebsocketConfig {
    /// Builder-style setter.
    pub fn scheme(mut self, scheme: &'static str) -> Self {
        self.scheme = scheme;
        self
    }

    /// Builder-style setter.
    pub fn default_request_timeout_ms(mut self, ms: u64) -> Self {
        self.default_request_timeout_ms = ms;
        self
    }

    /// Builder-style setter.
    pub fn latency_disconnect_ms(mut self, ms: Option<u64>) -> Self {
        self.latency_disconnect_ms = ms;
        self
    }

    /// Builder-style setter.
    pub fn max_send_queue(mut self, max: usize) -> Self {
        self.max_send_queue = max;
        self
    }

    /// Builder-style setter.
    pub fn max_message_size(mut self, max: usize) -> Self {
        self.max_message_size = max;
        self
    }

    /// Builder-style setter.
    pub fn max_frame_size(mut self, max: usize) -> Self {
        self.max_frame_size = max;
        self
    }
}
