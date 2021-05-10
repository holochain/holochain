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
    pub base: Arc<EntryHash>,
    pub zome_id: ZomeId,
    pub tag: Option<Arc<String>>,
    query: Arc<String>,
}

impl LinksQuery {
    pub fn new(base: EntryHash, zome_id: ZomeId, tag: Option<LinkTag>) -> Self {
        let tag = tag.map(|tag| Arc::new(Self::tag_to_hex(&tag)));
        let create_string = Self::create_query_string(tag.clone());
        let delete_string = Self::delete_query_string(tag.clone());
        Self {
            base: Arc::new(base),
            zome_id,
            tag,
            query: Arc::new(Self::create_query(create_string, delete_string)),
        }
    }

    pub fn tag_to_hex(tag: &LinkTag) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(tag.0.len());
        for b in &tag.0 {
            write!(&mut s, "{:X}", b).ok();
        }
        s
    }

    pub fn base(base: EntryHash, zome_id: ZomeId) -> Self {
        Self::new(base, zome_id, None)
    }

    pub fn tag(base: EntryHash, zome_id: ZomeId, tag: LinkTag) -> Self {
        Self::new(base, zome_id, Some(tag))
    }

    fn create_query(create: String, delete: String) -> String {
        format!("{} UNION ALL {}", create, delete)
    }

    pub fn query(&self) -> String {
        (*self.query).clone()
    }

    fn common_query_string() -> &'static str {
        "
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :create
            AND
            Header.base_hash = :base_hash
            AND
            Header.zome_id = :zome_id
            AND
            DhtOp.validation_status = :status
            AND (DhtOp.when_integrated IS NOT NULL OR DhtOp.is_authored = 1)
        "
    }
    fn create_query_string(tag: Option<Arc<String>>) -> String {
        let s = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        Self::add_tag(s, tag)
    }
    fn add_tag(q: String, tag: Option<Arc<String>>) -> String {
        if let Some(tag) = tag {
            format!(
                "{}
            AND
            HEX(Header.tag) like '{}%'",
                q, tag
            )
        } else {
            q
        }
    }
    fn delete_query_string(tag: Option<Arc<String>>) -> String {
        let sub_create_query = format!(
            "
            SELECT Header.hash FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        let sub_create_query = Self::add_tag(sub_create_query, tag);
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
            (DhtOp.when_integrated IS NOT NULL OR DhtOp.is_authored = 1)
            ",
            sub_create_query
        );
        delete_query
    }

    pub fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":delete": DhtOpType::RegisterRemoveLink,
            ":base_hash": self.base,
            ":zome_id": self.zome_id,
            ":status": ValidationStatus::Valid,
        }
        .to_vec();
        params
    }
}

impl GetLinksQuery {
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
                    && tag_filter
                        .as_ref()
                        .map(|t| LinksQuery::tag_to_hex(tag).starts_with(&(**t)))
                        .unwrap_or(true)
            }
            Header::DeleteLink(DeleteLink { base_address, .. }) => *base_address == *base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let (header, _) = shh.into_header_and_signature();
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
