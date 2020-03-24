//! defines a builder-style config struct for setting up websockets

/// A builder-style config struct for setting up websockets.
#[derive(Debug)]
pub struct WebsocketConfig {
    /// Scheme to use for urls - e.g. "ws" or "wss". [default = "ws"]
    pub scheme: &'static str,

    /// Seconds after which the lib will stop tracking individual request ids.
    /// [default = 30]
    pub default_request_timeout_s: usize,

    /// We will treat the socket as disconnected if we receive no messages
    /// in this timeframe, using the tcp keepalive mechanism.
    /// [default = 10]
    pub tcp_keepalive_s: usize,

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
            default_request_timeout_s: 30,
            tcp_keepalive_s: 30,
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
    pub fn default_request_timeout_s(mut self, s: usize) -> Self {
        self.default_request_timeout_s = s;
        self
    }

    /// Builder-style setter.
    pub fn tcp_keepalive_s(mut self, s: usize) -> Self {
        self.tcp_keepalive_s = s;
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
