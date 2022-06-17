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
    pub fn new(
        base: AnyLinkableHash,
        type_query: Option<LinkTypeRanges>,
        tag: Option<LinkTag>,
    ) -> Self {
        Self {
            query: LinksQuery::new(base, type_query, tag),
        }
    }
}

impl Query for GetLinkDetailsQuery {
    type Item = Judged<SignedActionHashed>;
    type State = HashMap<ActionHash, (Option<SignedActionHashed>, HashSet<SignedActionHashed>)>;
    type Output = Vec<(SignedActionHashed, Vec<SignedActionHashed>)>;
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
        let f = row_blob_to_action("action_blob");
        // Data is valid because it is filtered in the sql query.
        Arc::new(move |row| Ok(Judged::valid(f(row)?)))
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let query = &self.query;
        let base_filter = query.base.clone();
        let type_query_filter = query.type_query.clone();
        let tag_filter = query.tag.clone();
        let f = move |action: &QueryData<Self>| match action.action() {
            Action::CreateLink(CreateLink {
                base_address,
                tag,
                link_type,
                ..
            }) => {
                *base_address == *base_filter
                    && type_query_filter
                        .as_ref()
                        .map_or(true, |z| z.contains(link_type))
                    && tag_filter
                        .as_ref()
                        .map_or(true, |t| LinksQuery::tag_to_hex(tag).starts_with(&(**t)))
            }
            Action::DeleteLink(DeleteLink { base_address, .. }) => *base_address == *base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let action = shh.action();
        match action {
            Action::CreateLink(_) => {
                state
                    .entry(shh.as_hash().clone())
                    .or_insert((Some(shh), HashSet::new()));
            }
            Action::DeleteLink(delete_link) => {
                let entry = state
                    .entry(delete_link.link_add_address.clone())
                    .or_insert((None, HashSet::new()));
                entry.1.insert(shh);
            }
            _ => {
                return Err(StateQueryError::UnexpectedAction(
                    shh.action().action_type(),
                ))
            }
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
                    deletes.sort_by_key(|l| l.action().timestamp());
                    (create, deletes.into_iter().collect())
                })
            })
            .collect::<Vec<_>>();
        r.sort_by_key(|l| l.0.action().timestamp());
        Ok(r)
    }
}
