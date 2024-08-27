use holo_hash::*;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::*;
use holochain_state::{prelude::*, query::QueryData};
use std::fmt::Debug;
use std::sync::Arc;
use crate::authority::get_agent_activity_query::{fold, render, Item, State};

#[derive(Debug, Clone)]
pub struct GetAgentActivityRecordsQuery {
    pub(super) agent: AgentPubKey,
    pub(super) agent_basis: AnyLinkableHash,
    pub(super) filter: ChainQueryFilter,
    pub(super) options: GetActivityOptions,
}

impl GetAgentActivityRecordsQuery {
    pub fn new(agent: AgentPubKey, filter: ChainQueryFilter, options: GetActivityOptions) -> Self {
        Self {
            agent_basis: agent.clone().into(),
            agent,
            filter,
            options,
        }
    }
}

impl Query for GetAgentActivityRecordsQuery {
    type State = State<Record>;
    type Item = Judged<Item<Record>>;
    type Output = AgentActivityResponse;

    fn query(&self) -> String {
        "
            SELECT
            Action.hash,
            Action.blob AS action_blob,
            DhtOp.type AS dht_type,
            DhtOp.validation_status,
            DhtOp.when_integrated,
            Entry.blob AS entry_blob
            FROM Action
            JOIN DhtOp ON DhtOp.action_hash = Action.hash
            LEFT JOIN Entry ON Action.entry_hash = Entry.hash
            WHERE
            (
                -- is an action authored by this agent
                Action.author = :author
                AND DhtOp.type = :chain_op_type
            )
            OR
            (
                -- is an integrated, valid warrant
                DhtOp.basis_hash = :author_basis
                AND DhtOp.type = :warrant_op_type
                AND DhtOp.validation_status = :valid_status
                AND DhtOp.when_integrated IS NOT NULL
            )
            ORDER BY Action.seq ASC
        "
        .to_string()
    }

    fn params(&self) -> Vec<holochain_state::query::Params> {
        named_params! {
            ":author": self.agent,
            ":author_basis": self.agent_basis,
            ":chain_op_type": ChainOpType::RegisterAgentActivity,
            ":warrant_op_type": WarrantOpType::ChainIntegrityWarrant,
            ":valid_status": ValidationStatus::Valid,
        }
        .to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(Default::default())
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        unimplemented!("This query should not be used with the scratch")
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        Arc::new(move |row| {
            let op_type: DhtOpType = row.get("dht_type")?;
            let validation_status: Option<ValidationStatus> = row.get("validation_status")?;
            let integrated: Option<Timestamp> = row.get("when_integrated")?;

            // Note that even if an entry is expected, it might not be present.
            // This code is intended to run on an agent authority which may not be an entry authority
            // for all the relevant entries. The caller will need to handle any missing entries that
            // they care about.
            let maybe_entry = row.get::<_, Option<Vec<u8>>>("entry_blob")?.map(from_blob).transpose()?;

            match op_type {
                DhtOpType::Chain(_) => {
                    let hash: ActionHash = row.get("hash")?;
                    from_blob::<SignedAction>(row.get("action_blob")?).map(|action| {
                        let action = SignedHashed {
                            hashed: ActionHashed::with_pre_hashed(action.action().clone(), hash),
                            signature: action.signature().clone(),
                        };
                        let record = Record::new(action, maybe_entry);
                        let item = if integrated.is_some() {
                            Item::Integrated(record)
                        } else {
                            Item::Pending(record)
                        };
                        Judged::raw(item, validation_status)
                    })
                }
                DhtOpType::Warrant(_) => {
                    let _hash: WarrantHash = row.get("hash")?;
                    from_blob::<SignedWarrant>(row.get("action_blob")?).map(|warrant| {
                        let item = Item::Warrant(warrant.into_data());
                        Judged::raw(item, None)
                    })
                }
            }
        })
    }

    fn fold(&self, state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        fold(state, item)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        render(state, self.agent.clone(), &self.filter, &self.options)
    }
}
