use holo_hash::AgentPubKey;
use holochain_types::link::WireLinkQuery;
use holochain_types::prelude::Timestamp;

#[derive(Debug, Clone, Default)]
pub struct GetLinksFilter {
    pub after: Option<Timestamp>,
    pub before: Option<Timestamp>,
    pub author: Option<AgentPubKey>,
}

impl From<WireLinkQuery> for GetLinksFilter {
    fn from(value: WireLinkQuery) -> Self {
        Self {
            before: value.before,
            after: value.after,
            author: value.author,
        }
    }
}
