use holo_hash::*;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::link::LinksQuery;
use super::*;

#[derive(Debug, Clone)]
pub struct GetLinkDetailsQuery {
    query: LinksQuery,
}

impl GetLinkDetailsQuery {
    pub fn new(base: EntryHash, zome_id: ZomeId, tag: Option<LinkTag>) -> Self {
        Self {
            query: LinksQuery::new(base, zome_id, tag),
        }
    }

    pub fn base(base: EntryHash, zome_id: ZomeId) -> Self {
        Self {
            query: LinksQuery::base(base, zome_id),
        }
    }

    pub fn tag(base: EntryHash, zome_id: ZomeId, tag: LinkTag) -> Self {
        Self {
            query: LinksQuery::tag(base, zome_id, tag),
        }
    }
}

impl Query for GetLinkDetailsQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = HashMap<HeaderHash, (Option<SignedHeaderHashed>, HashSet<SignedHeaderHashed>)>;
    type Output = Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)>;
    fn query(&self) -> String {
        self.query.query()
    }

    fn params(&self) -> Vec<Params> {
        self.query.params()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(HashMap::new())
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = row_blob_to_header("header_blob");
        // Data is valid because it is filtered in the sql query.
        Arc::new(move |row| Ok(Judged::valid(f(row)?)))
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let query = &self.query;
        let base_filter = query.base.clone();
        let zome_id_filter = query.zome_id;
        let tag_filter = query.tag.clone();
        let f = move |header: &QueryData<Self>| match header.header() {
            Header::CreateLink(CreateLink {
                base_address,
                zome_id,
                tag,
                ..
            }) => {
                *base_address == *base_filter
                    && *zome_id == zome_id_filter
                    && tag_filter.as_ref().map(|t| *tag == **t).unwrap_or(true)
            }
            Header::DeleteLink(DeleteLink { base_address, .. }) => *base_address == *base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let header = shh.header();
        match header {
            Header::CreateLink(_) => {
                state
                    .entry(shh.as_hash().clone())
                    .or_insert((Some(shh), HashSet::new()));
            }
            Header::DeleteLink(delete_link) => {
                let entry = state
                    .entry(delete_link.link_add_address.clone())
                    .or_insert((None, HashSet::new()));
                entry.1.insert(shh);
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        // TODO: This could be done above by using BTMaps but deferring this optimization
        // because it's simpler .
        // Order by timestamp.
        let mut r = state
            .into_iter()
            .filter_map(|(_, (create, deletes))| {
                create.map(|create| {
                    let mut deletes = deletes.into_iter().collect::<Vec<_>>();
                    deletes.sort_by_key(|l| l.header().timestamp());
                    (create, deletes.into_iter().collect())
                })
            })
            .collect::<Vec<_>>();
        r.sort_by_key(|l| l.0.header().timestamp());
        Ok(r)
    }
}
