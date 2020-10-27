use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
/// Configure which network to use
pub enum NetworkConfig {
    /// The Sim2h netowrk
    Sim2h {
        #[serde(with = "url_serde")]
        /// Which url the sim2h server is running on
        url: Url,
    },
}
