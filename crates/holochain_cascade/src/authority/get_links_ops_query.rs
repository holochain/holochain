use std::sync::Arc;

use holo_hash::AnyLinkableHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::sql::ToSqlStatement;

use super::WireLinkKey;

#[derive(Debug, Clone)]
pub struct GetLinksOpsQuery {
    base: Arc<AnyLinkableHash>,
    type_query: LinkTypeFilter,
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
        holochain_util::hex::bytes_to_hex(&tag.0, true)
    }
}

pub struct Item {
    action: SignedAction,
    op_type: ChainOpType,
}

impl Query for GetLinksOpsQuery {
    type Item = Judged<Item>;
    type State = WireLinkOps;
    type Output = Self::State;

    fn query(&self) -> String {
        let create = "
            SELECT Action.blob AS action_blob, DhtOp.type AS dht_type,
            DhtOp.validation_status AS status
            FROM DhtOp
        ";
        let sub_create = "
            SELECT Action.hash FROM DhtOp
        ";
        let mut common_query = "
            JOIN Action On DhtOp.action_hash = Action.hash
            WHERE DhtOp.type = :create
            AND
            Action.base_hash = :base_hash
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
                    HEX(Action.tag) LIKE '{}%'
                ",
                common_query, tag
            );
        }
        common_query = format!(
            "
            {}
            {}
            ",
            common_query,
            self.type_query.to_sql_statement(),
        );
        let create_query = format!("{}{}", create, common_query);
        let sub_create_query = format!("{}{}", sub_create, common_query);
        let delete_query = format!(
            "
            SELECT Action.blob AS action_blob, DhtOp.type AS dht_type,
            DhtOp.validation_status AS status
            FROM DhtOp
            JOIN Action On DhtOp.action_hash = Action.hash
            WHERE DhtOp.type = :delete
            AND
            DhtOp.when_integrated IS NOT NULL
            AND
            Action.create_link_hash IN ({})
            ",
            sub_create_query
        );
        format!("{} UNION ALL {}", create_query, delete_query)
    }

    fn params(&self) -> Vec<Params> {
        named_params! {
            ":create": ChainOpType::RegisterAddLink,
            ":delete": ChainOpType::RegisterRemoveLink,
            ":base_hash": self.base,
        }
        .to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = |row: &Row| {
            let action =
                from_blob::<SignedAction>(row.get(row.as_ref().column_index("action_blob")?)?)?;
            let op_type = row.get(row.as_ref().column_index("dht_type")?)?;
            let validation_status = row.get(row.as_ref().column_index("status")?)?;
            Ok(Judged::raw(Item { action, op_type }, validation_status))
        };
        Arc::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(WireLinkOps::new())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Item) -> StateQueryResult<Self::State> {
        match &dht_op.data.op_type {
            ChainOpType::RegisterAddLink => {
                let validation_status = dht_op.validation_status();
                let item = dht_op.data.action;
                if let (
                    SignedAction(Action::CreateLink(action), signature),
                    Some(validation_status),
                ) = (item, validation_status)
                {
                    state.creates.push(WireCreateLink::condense(
                        action,
                        signature,
                        validation_status,
                    ));
                }
            }
            ChainOpType::RegisterRemoveLink => {
                let validation_status = dht_op.validation_status();
                let item = dht_op.data.action;
                if let (
                    SignedAction(Action::DeleteLink(action), signature),
                    Some(validation_status),
                ) = (item, validation_status)
                {
                    state.deletes.push(WireDeleteLink::condense(
                        action,
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
