use serde::{Deserialize, Serialize};

/// FIXME: implement
#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoggerConfig {}
