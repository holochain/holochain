//! Strategy for fetching data from local or network sources.

use serde::{Deserialize, Serialize};

/// Set if data should be fetched from the network or only from the local
/// databases.
#[derive(PartialEq, Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GetStrategy {
    /// Fetch latest metadata from the network,
    /// and otherwise fall back to locally cached metadata.
    ///
    /// If the current agent is an authority for this hash, this call will not
    /// go to the network.
    #[default]
    Network,
    /// Gets the action/entry and its metadata from local databases only.
    /// No network call is made.
    Local,
}
