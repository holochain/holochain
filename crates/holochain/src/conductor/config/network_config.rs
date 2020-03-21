use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum NetworkConfig {
    Sim2h {
        #[serde(with = "url_serde")]
        url: Url
    }
}
