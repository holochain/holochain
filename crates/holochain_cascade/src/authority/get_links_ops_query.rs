use std::sync::Arc;

use holo_hash::AnyLinkableHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::dht_op::DhtOpType;
use holochain_types::link::WireCreateLink;
use holochain_types::link::WireDeleteLink;
use holochain_types::link::WireLinkOps;
use holochain_zome_types::HasValidationStatus;
use holochain_zome_types::Header;
use holochain_zome_types::Judged;
use holochain_zome_types::LinkTag;
use holochain_zome_types::LinkTypeQuery;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::ZomeId;

use super::WireLinkKey;

#[derive(Debug, Clone)]
pub struct GetLinksOpsQuery {
    base: Arc<AnyLinkableHash>,
    type_query: Option<LinkTypeQuery<ZomeId>>,
    tag: Option<Arc<LinkTag>>,
}

impl GetLinksOpsQuery {
    pub fn new(key: WireLinkKey) -> Self {
        Self {
            base: Arc::new(key.base),
            type_query: key.type_query,
            tag: key.tag.map(Arc::new),
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
}

pub struct Item {
    header: SignedHeader,
    op_type: DhtOpType,
}

impl Query for GetLinksOpsQuery {
    type Item = Judged<Item>;
    type State = WireLinkOps;
    type Output = Self::State;

    fn query(&self) -> String {
        let create = "
            SELECT Header.blob AS header_blob, DhtOp.type AS dht_type,
            DhtOp.validation_status AS status
            FROM DhtOp
        ";
        let sub_create = "
            SELECT Header.hash FROM DhtOp
        ";
        let mut common_query = "
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :create
            AND
            Header.base_hash = :base_hash
            AND
            DhtOp.when_integrated IS NOT NULL
        "
        .to_string();

        if let Some(tag) = &self.tag {
            let tag = Self::tag_to_hex(tag.as_ref());
            common_query = format!(
                "
                    {}
                    AND
                    HEX(Header.tag) LIKE '{}%'
                ",
                common_query, tag
            );
        }
        if let Some(link_type) = &self.type_query {
            common_query = format!(
                "
                {}
                AND
                Header.zome_id = :zome_id
                ",
                common_query
            );
            if let LinkTypeQuery::SingleType(_, _) = link_type {
                common_query = format!(
                    "
                {}
                AND
                Header.link_type = :link_type
                ",
                    common_query
                );
            }
        }
        let create_query = format!("{}{}", create, common_query);
        let sub_create_query = format!("{}{}", sub_create, common_query);
        let delete_query = format!(
            "
            SELECT Header.blob AS header_blob, DhtOp.type AS dht_type,
            DhtOp.validation_status AS status
            FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :delete
            AND
            DhtOp.when_integrated IS NOT NULL
            AND
            Header.create_link_hash IN ({})
            ",
            sub_create_query
        );
        format!("{} UNION ALL {}", create_query, delete_query)
    }

    fn params(&self) -> Vec<Params> {
        match &self.type_query {
            Some(LinkTypeQuery::AllTypes(zome_id)) => {
                named_params! {
                    ":create": DhtOpType::RegisterAddLink,
                    ":delete": DhtOpType::RegisterRemoveLink,
                    ":base_hash": self.base,
                    ":zome_id": **zome_id,
                }
            }
            .to_vec(),
            Some(LinkTypeQuery::SingleType(zome_id, link_type)) => {
                named_params! {
                    ":create": DhtOpType::RegisterAddLink,
                    ":delete": DhtOpType::RegisterRemoveLink,
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
                    ":base_hash": self.base,
                }
            }
            .to_vec(),
        }
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = |row: &Row| {
            let header =
                from_blob::<SignedHeader>(row.get(row.as_ref().column_index("header_blob")?)?)?;
            let op_type = row.get(row.as_ref().column_index("dht_type")?)?;
            let validation_status = row.get(row.as_ref().column_index("status")?)?;
            Ok(Judged::raw(Item { header, op_type }, validation_status))
        };
        Arc::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(WireLinkOps::new())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Item) -> StateQueryResult<Self::State> {
        match &dht_op.data.op_type {
            DhtOpType::RegisterAddLink => {
                let validation_status = dht_op.validation_status();
                let item = dht_op.data.header;
                if let (
                    SignedHeader(Header::CreateLink(header), signature),
                    Some(validation_status),
                ) = (item, validation_status)
                {
                    state.creates.push(WireCreateLink::condense(
                        header,
                        signature,
                        validation_status,
                    ));
                }
            }
            DhtOpType::RegisterRemoveLink => {
                let validation_status = dht_op.validation_status();
                let item = dht_op.data.header;
                if let (
                    SignedHeader(Header::DeleteLink(header), signature),
                    Some(validation_status),
                ) = (item, validation_status)
                {
                    state.deletes.push(WireDeleteLink::condense(
                        header,
                        signature,
                        validation_status,
                    ));
                }
            }
            _ => return Err(StateQueryError::UnexpectedOp(dht_op.data.op_type)),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(state)
    }
}
