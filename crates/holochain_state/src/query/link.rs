use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::ChainOpType;
use holochain_types::sql::ToSqlStatement;
use holochain_zome_types::prelude::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetLinksQuery {
    query: LinksQuery,
}

#[derive(Debug, Clone, Default)]
pub struct GetLinksFilter {
    pub after: Option<Timestamp>,
    pub before: Option<Timestamp>,
    pub author: Option<AgentPubKey>,
}

#[derive(Debug, Clone)]
pub struct LinksQuery {
    pub base: Arc<AnyLinkableHash>,
    pub type_query: LinkTypeFilter,
    pub tag: Option<String>,
    filter: GetLinksFilter,
    query: String,
}

impl LinksQuery {
    pub fn new(
        base: AnyLinkableHash,
        type_query: LinkTypeFilter,
        tag: Option<LinkTag>,
        filter: GetLinksFilter,
    ) -> Self {
        let tag = tag.map(|tag| Self::tag_to_hex(&tag));
        let create_string = Self::create_query_string(&type_query, tag.clone(), &filter);
        let delete_string = Self::delete_query_string(&type_query, tag.clone());
        Self {
            base: Arc::new(base),
            type_query,
            tag,
            filter,
            query: Self::create_query(create_string, delete_string),
        }
    }

    pub fn tag_to_hex(tag: &LinkTag) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(tag.0.len());
        for b in &tag.0 {
            write!(&mut s, "{:02X}", b).ok();
        }
        s
    }

    pub fn base(base: AnyLinkableHash, dependencies: Vec<ZomeIndex>) -> Self {
        Self::new(
            base,
            LinkTypeFilter::Dependencies(dependencies),
            None,
            GetLinksFilter::default(),
        )
    }

    fn create_query(create: String, delete: String) -> String {
        format!("{} UNION ALL {}", create, delete)
    }

    pub fn query(&self) -> String {
        self.query.clone()
    }

    fn common_query_string() -> &'static str {
        "
            JOIN Action On DhtOp.action_hash = Action.hash
            WHERE DhtOp.type = :create
            AND
            Action.base_hash = :base_hash
            AND
            DhtOp.validation_status = :status
            AND DhtOp.when_integrated IS NOT NULL
        "
    }

    fn create_query_string(
        type_query: &LinkTypeFilter,
        tag: Option<String>,
        filter: &GetLinksFilter,
    ) -> String {
        let mut s = format!(
            "
            SELECT Action.blob AS action_blob FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        s = Self::add_type_query(s, type_query);
        s = Self::add_tag(s, tag);
        s = Self::add_after(s, filter.after);
        s = Self::add_before(s, filter.before);
        s = Self::add_author(s, filter.author.as_ref());

        s
    }

    fn add_type_query(q: String, type_query: &LinkTypeFilter) -> String {
        format!("{} {} ", q, type_query.to_sql_statement())
    }

    fn add_tag(q: String, tag: Option<String>) -> String {
        match tag {
            Some(tag) => {
                format!(
                    "{}
                    AND
                    HEX(Action.tag) like '{}%'",
                    q, tag
                )
            }
            None => q,
        }
    }

    fn add_after(q: String, after: Option<Timestamp>) -> String {
        match after {
            Some(_) => format!("{} AND DhtOp.authored_timestamp >= :after", q),
            None => format!("{} AND :after IS NULL", q),
        }
    }

    fn add_before(q: String, before: Option<Timestamp>) -> String {
        match before {
            Some(_) => format!("{} AND DhtOp.authored_timestamp <= :before", q),
            None => format!("{} AND :before IS NULL", q),
        }
    }

    fn add_author(q: String, author: Option<&AgentPubKey>) -> String {
        match author {
            Some(_) => format!("{} AND Action.author = :author", q),
            None => format!("{} AND :author IS NULL", q),
        }
    }

    fn delete_query_string(type_query: &LinkTypeFilter, tag: Option<String>) -> String {
        let mut sub_create_query = format!(
            "
            SELECT Action.hash FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        sub_create_query = Self::add_type_query(sub_create_query, type_query);
        sub_create_query = Self::add_tag(sub_create_query, tag);
        let delete_query = format!(
            "
            SELECT Action.blob AS action_blob FROM DhtOp
            JOIN Action On DhtOp.action_hash = Action.hash
            WHERE DhtOp.type = :delete
            AND
            Action.create_link_hash IN ({})
            AND
            DhtOp.validation_status = :status
            AND
            DhtOp.when_integrated IS NOT NULL
            ",
            sub_create_query
        );
        delete_query
    }

    pub fn params(&self) -> Vec<Params> {
        {
            named_params! {
                ":create": ChainOpType::RegisterAddLink,
                ":delete": ChainOpType::RegisterRemoveLink,
                ":status": ValidationStatus::Valid,
                ":base_hash": self.base,
                ":after": self.filter.after,
                ":before": self.filter.before,
                ":author": self.filter.author,
            }
        }
        .to_vec()
    }
}

impl GetLinksQuery {
    pub fn new(
        base: AnyLinkableHash,
        type_query: LinkTypeFilter,
        tag: Option<LinkTag>,
        filter: GetLinksFilter,
    ) -> Self {
        Self {
            query: LinksQuery::new(base, type_query, tag, filter),
        }
    }

    pub fn base(base: AnyLinkableHash, dependencies: Vec<ZomeIndex>) -> Self {
        Self {
            query: LinksQuery::base(base, dependencies),
        }
    }
}

impl Query for GetLinksQuery {
    type Item = Judged<SignedActionHashed>;
    type State = Maps<Link>;
    type Output = Vec<Link>;
    fn query(&self) -> String {
        self.query.query()
    }

    fn params(&self) -> Vec<Params> {
        self.query.params()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(Maps::new())
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
                zome_index,
                link_type,
                ..
            }) => {
                *base_address == *base_filter
                    && type_query_filter.contains(zome_index, link_type)
                    && tag_filter
                        .as_ref()
                        .is_none_or(|t| LinksQuery::tag_to_hex(tag).starts_with(&(**t)))
            }
            Action::DeleteLink(DeleteLink { base_address, .. }) => *base_address == *base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let (action, _) = shh.into_inner();
        let (action, hash) = action.into_inner();
        match action {
            Action::CreateLink(create_link) => {
                if !state.deletes.contains(&hash) {
                    state
                        .creates
                        .insert(hash, link_from_action(Action::CreateLink(create_link))?);
                }
            }
            Action::DeleteLink(delete_link) => {
                state.creates.remove(&delete_link.link_add_address);
                state.deletes.insert(delete_link.link_add_address);
            }
            _ => return Err(StateQueryError::UnexpectedAction(action.action_type())),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let mut links: Self::Output = state.creates.into_values().collect();
        links.sort_by_key(|l| l.timestamp);
        Ok(links)
    }
}

fn link_from_action(action: Action) -> StateQueryResult<Link> {
    let hash = ActionHash::with_data_sync(&action);
    match action {
        Action::CreateLink(action) => Ok(Link {
            author: action.author,
            base: action.base_address,
            target: action.target_address,
            timestamp: action.timestamp,
            zome_index: action.zome_index,
            link_type: action.link_type,
            tag: action.tag,
            create_link_hash: hash,
        }),
        _ => Err(StateQueryError::UnexpectedAction(action.action_type())),
    }
}
