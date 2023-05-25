use crate::query::link::GetLinksFilter;
use holochain_types::link::WireLinkQuery;

// Note that link_count uses `GetLinksQuery`, so there is no query implemented here

impl From<WireLinkQuery> for GetLinksFilter {
    fn from(value: WireLinkQuery) -> Self {
        Self {
            before: value.before,
            after: value.after,
            author: value.author,
        }
    }
}
