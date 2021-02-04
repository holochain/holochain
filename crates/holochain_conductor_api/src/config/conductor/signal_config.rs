use serde::{self, Deserialize, Serialize};

/// Configure which signals to emit, to reduce unwanted signal volume
#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct SignalConfig {
    pub trace: bool,
    pub consistency: bool,
}
