use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetLinksQuery {
    query: LinksQuery,
}

#[derive(Debug, Clone)]
pub struct LinksQuery {
    pub base: Arc<AnyLinkableHash>,
    pub type_query: Option<LinkTypeQuery<ZomeId>>,
    pub tag: Option<String>,
    query: String,
}

impl LinksQuery {
    pub fn new(
        base: AnyLinkableHash,
        type_query: Option<LinkTypeQuery<ZomeId>>,
        tag: Option<LinkTag>,
    ) -> Self {
        let tag = tag.map(|tag| Self::tag_to_hex(&tag));
        let create_string = Self::create_query_string(&type_query, tag.clone());
        let delete_string = Self::delete_query_string(&type_query, tag.clone());
        Self {
            base: Arc::new(base),
            type_query,
            tag,
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

    pub fn base(base: AnyLinkableHash) -> Self {
        Self::new(base, None, None)
    }

    fn create_query(create: String, delete: String) -> String {
        format!("{} UNION ALL {}", create, delete)
    }

    pub fn query(&self) -> String {
        self.query.clone()
    }

    fn common_query_string() -> &'static str {
        "
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :create
            AND
            Header.base_hash = :base_hash
            AND
            DhtOp.validation_status = :status
            AND DhtOp.when_integrated IS NOT NULL
        "
    }
    fn create_query_string(
        type_query: &Option<LinkTypeQuery<ZomeId>>,
        tag: Option<String>,
    ) -> String {
        let mut s = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        s = Self::add_query(s, type_query);
        Self::add_tag(s, tag)
    }
    fn add_tag(q: String, tag: Option<String>) -> String {
        match tag {
            Some(tag) => {
                format!(
                    "{}
                    AND
                    HEX(Header.tag) like '{}%'",
                    q, tag
                )
            }
            None => q,
        }
    }
    fn add_query(mut q: String, type_query: &Option<LinkTypeQuery<ZomeId>>) -> String {
        if let Some(link_type) = type_query {
            q = format!(
                "{}
                AND
                Header.zome_id = :zome_id
                ",
                q
            );
            if let LinkTypeQuery::SingleType(_, _) = link_type {
                q = format!(
                    "{}
                    AND
                    Header.link_type = :link_type
                    ",
                    q
                );
            }
        }
        q
    }
    fn delete_query_string(
        type_query: &Option<LinkTypeQuery<ZomeId>>,
        tag: Option<String>,
    ) -> String {
        let mut sub_create_query = format!(
            "
            SELECT Header.hash FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        sub_create_query = Self::add_query(sub_create_query, type_query);
        sub_create_query = Self::add_tag(sub_create_query, tag);
        let delete_query = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :delete
            AND
            Header.create_link_hash IN ({})
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
        match &self.type_query {
            Some(LinkTypeQuery::AllTypes(zome_id)) => {
                named_params! {
                    ":create": DhtOpType::RegisterAddLink,
                    ":delete": DhtOpType::RegisterRemoveLink,
                    ":status": ValidationStatus::Valid,
                    ":base_hash": self.base,
                    ":zome_id": **zome_id,
                }
            }
            .to_vec(),
            Some(LinkTypeQuery::SingleType(zome_id, link_type)) => {
                named_params! {
                    ":create": DhtOpType::RegisterAddLink,
                    ":delete": DhtOpType::RegisterRemoveLink,
                    ":status": ValidationStatus::Valid,
                    ":base_hash": self.base,
                    ":zome_id": **zome_id,
                    ":link_type": **link_type,
                }
            }
            .to_vec(),
            None => {
                named_params! {
                    ":create": DhtOpType::RegisterAddLink,
                    ":delete": DhtOpType::RegisterRemoveLink,
                    ":status": ValidationStatus::Valid,
                    ":base_hash": self.base,
                }
            }
            .to_vec(),
        }
    }
}

impl GetLinksQuery {
    pub fn new(
        base: AnyLinkableHash,
        type_query: Option<LinkTypeQuery<ZomeId>>,
        tag: Option<LinkTag>,
    ) -> Self {
        Self {
            query: LinksQuery::new(base, type_query, tag),
        }
    }

    pub fn base(base: AnyLinkableHash) -> Self {
        Self {
            query: LinksQuery::base(base),
        }
    }
}

impl Query for GetLinksQuery {
    type Item = Judged<SignedHeaderHashed>;
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
        let f = row_blob_to_header("header_blob");
        // Data is valid because it is filtered in the sql query.
        Arc::new(move |row| Ok(Judged::valid(f(row)?)))
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let query = &self.query;
        let base_filter = query.base.clone();
        let type_query_filter = query.type_query.clone();
        let tag_filter = query.tag.clone();
        let f = move |header: &QueryData<Self>| match header.header() {
            Header::CreateLink(CreateLink {
                base_address,
                zome_id,
                tag,
                link_type,
                ..
            }) => {
                *base_address == *base_filter
                    && type_query_filter.as_ref().map_or(true, |z| match z {
                        LinkTypeQuery::AllTypes(z) => *zome_id == *z,
                        LinkTypeQuery::SingleType(z, lt) => *zome_id == *z && *link_type == *lt,
                    })
                    && tag_filter
                        .as_ref()
                        .map_or(true, |t| LinksQuery::tag_to_hex(tag).starts_with(&(**t)))
            }
            Header::DeleteLink(DeleteLink { base_address, .. }) => *base_address == *base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let (header, _) = shh.into_inner();
        let (header, hash) = header.into_inner();
        match header {
            Header::CreateLink(create_link) => {
                if !state.deletes.contains(&hash) {
                    state
                        .creates
                        .insert(hash, link_from_header(Header::CreateLink(create_link))?);
                }
            }
            Header::DeleteLink(delete_link) => {
                state.creates.remove(&delete_link.link_add_address);
                state.deletes.insert(delete_link.link_add_address);
            }
            _ => return Err(StateQueryError::UnexpectedHeader(header.header_type())),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let mut links: Self::Output = state.creates.into_iter().map(|(_, v)| v).collect();
        links.sort_by_key(|l| l.timestamp);
        Ok(links)
    }
}

fn link_from_header(header: Header) -> StateQueryResult<Link> {
    let hash = HeaderHash::with_data_sync(&header);
    match header {
        Header::CreateLink(header) => Ok(Link {
            target: header.target_address,
            timestamp: header.timestamp,
            tag: header.tag,
            create_link_hash: hash,
        }),
        _ => Err(StateQueryError::UnexpectedHeader(header.header_type())),
    }
}
