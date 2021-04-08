use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct LinkQuery {
    base: EntryHash,
    zome_id: ZomeId,
    tag: Option<LinkTag>,
    create_string: String,
    delete_string: String,
}

impl LinkQuery {
    fn new(base: EntryHash, zome_id: ZomeId, tag: Option<LinkTag>) -> Self {
        Self {
            base,
            zome_id,
            create_string: Self::create_query_string(tag.is_some()),
            delete_string: Self::delete_query_string(tag.is_some()),
            tag,
        }
    }

    pub fn base(base: EntryHash, zome_id: ZomeId) -> Self {
        Self::new(base, zome_id, None)
    }

    pub fn tag(base: EntryHash, zome_id: ZomeId, tag: LinkTag) -> Self {
        Self::new(base, zome_id, Some(tag))
    }

    fn create_query(&self) -> &str {
        &self.create_string
    }

    fn delete_query(&self) -> &str {
        &self.delete_string
    }

    fn common_query_string() -> &'static str {
        "
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :create
            AND
            Header.base_hash = :base_hash
            AND
            Header.zome_id = :zome_id
        "
    }
    fn create_query_string(tag: bool) -> String {
        let s = format!(
            "
            SELECT Header.blob AS header_blob FROM DhtOp
            {}
            ",
            Self::common_query_string()
        );
        Self::add_tag(s, tag)
    }
    fn add_tag(q: String, tag: bool) -> String {
        if tag {
            format!(
                "{}
            AND
            Header.tag = :tag",
                q
            )
        } else {
            q
        }
    }
    fn delete_query_string(tag: bool) -> String {
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
            ",
            sub_create_query
        );
        delete_query
    }

    fn create_params(&self) -> Vec<Params> {
        let mut params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":base_hash": self.base,
            ":zome_id": self.zome_id,
        }
        .to_vec();
        if self.tag.is_some() {
            params.extend(named_params! {
                ":tag": self.tag,
            });
        }
        params
    }

    fn delete_params(&self) -> Vec<Params> {
        let mut params = named_params! {
            ":create": DhtOpType::RegisterAddLink,
            ":delete": DhtOpType::RegisterRemoveLink,
            ":base_hash": self.base,
            ":zome_id": self.zome_id,
        }
        .to_vec();
        if self.tag.is_some() {
            params.extend(named_params! {
                ":tag": self.tag,
            });
        }
        params
    }
}

impl Query for LinkQuery {
    type State = Maps<Link>;
    type Output = Vec<Link>;
    type Data = SignedHeaderHashed;
    fn create_query(&self) -> &str {
        self.create_query()
    }

    fn delete_query(&self) -> &str {
        self.delete_query()
    }

    fn create_params(&self) -> Vec<Params> {
        self.create_params()
    }

    fn delete_params(&self) -> Vec<Params> {
        self.delete_params()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(Maps::new())
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>> {
        Arc::new(row_blob_to_header("header_blob"))
    }

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        let base_filter = self.base.clone();
        let zome_id_filter = self.zome_id.clone();
        let tag_filter = self.tag.clone();
        let f = move |header: &SignedHeaderHashed| match header.header() {
            Header::CreateLink(CreateLink {
                base_address,
                zome_id,
                tag,
                ..
            }) => {
                *base_address == base_filter
                    && *zome_id == zome_id_filter
                    && tag_filter.as_ref().map(|t| tag == t).unwrap_or(true)
            }
            Header::DeleteLink(DeleteLink { base_address, .. }) => *base_address == base_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(
        &self,
        mut state: Self::State,
        shh: SignedHeaderHashed,
    ) -> StateQueryResult<Self::State> {
        let (header, _) = shh.into_header_and_signature();
        let (header, hash) = header.into_inner();
        match header {
            Header::CreateLink(create_link) => {
                if !state.deletes.contains(&hash) {
                    state
                        .creates
                        .insert(hash, link_from_header(Header::CreateLink(create_link)));
                }
            }
            Header::DeleteLink(delete_link) => {
                state.creates.remove(&delete_link.link_add_address);
                state.deletes.insert(delete_link.link_add_address);
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(state.creates.into_iter().map(|(_, v)| v).collect())
    }
}

fn link_from_header(header: Header) -> Link {
    let hash = HeaderHash::with_data_sync(&header);
    match header {
        Header::CreateLink(header) => Link {
            target: header.target_address,
            timestamp: header.timestamp,
            tag: header.tag,
            create_link_hash: hash,
        },
        _ => panic!("TODO: handle this properly"),
    }
}
