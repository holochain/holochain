pub use kitsune_p2p_gossip::bloom::*;

use crate::meta_net::MetaNetCon;

#[derive(Clone, Debug)]
pub(crate) enum HowToConnect {
    /// The connection handle and the url that this handle has been connected to.
    /// If the connection handle closes the url can change so we need to track it.
    Con(MetaNetCon, String),
    Url(String),
}
