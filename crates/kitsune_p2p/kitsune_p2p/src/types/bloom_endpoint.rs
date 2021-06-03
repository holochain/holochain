//! The endpoint for gossip communications

use crate::types::*;
use kitsune_p2p_types::Tx2Cert;

/// The specific provenance/destination of gossip is to a particular Agent on
/// a connection specified by a Tx2Cert
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Constructor)]
pub struct BloomEndpoint {
    agent: KitsuneAgent,
    cert: Tx2Cert,
}

impl BloomEndpoint {
    /// Accessor
    pub fn agent(&self) -> &KitsuneAgent {
        self.as_ref()
    }

    /// Accessor
    pub fn cert(&self) -> &Tx2Cert {
        self.as_ref()
    }
}

impl AsRef<KitsuneAgent> for BloomEndpoint {
    fn as_ref(&self) -> &KitsuneAgent {
        &self.agent
    }
}

impl AsRef<Tx2Cert> for BloomEndpoint {
    fn as_ref(&self) -> &Tx2Cert {
        &self.cert
    }
}
