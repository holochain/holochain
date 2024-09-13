use crate::authority::get_agent_activity_query::{fold, render, Item, State};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, WarrantHash};
use holochain_p2p::dht::op::Timestamp;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::{named_params, Row};
use holochain_state::prelude::{from_blob, ActionHashed, Query, StateQueryResult, Store};
use holochain_state::query::QueryData;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::dht_op::{ChainOpType, DhtOpType};
use holochain_types::prelude::WarrantOpType;
use holochain_zome_types::judged::Judged;
use holochain_zome_types::prelude::{
    ChainQueryFilter, SignedAction, SignedWarrant, ValidationStatus,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GetAgentActivityHashesQuery {
    pub(super) agent: AgentPubKey,
    pub(super) agent_basis: AnyLinkableHash,
    pub(super) filter: ChainQueryFilter,
    pub(super) options: GetActivityOptions,
}

impl GetAgentActivityHashesQuery {
    pub fn new(agent: AgentPubKey, filter: ChainQueryFilter, options: GetActivityOptions) -> Self {
        Self {
            agent_basis: agent.clone().into(),
            agent,
            filter,
            options,
        }
    }
}

impl Query for GetAgentActivityHashesQuery {
    type State = State<ActionHashed>;
    type Item = Judged<Item<ActionHashed>>;
    type Output = AgentActivityResponse;

    fn query(&self) -> String {
        "
            SELECT
            Action.hash,
            Action.blob AS action_blob,
            DhtOp.type AS dht_type,
            DhtOp.validation_status,
            DhtOp.when_integrated
            FROM Action
            JOIN DhtOp ON DhtOp.action_hash = Action.hash
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
        let params = named_params! {
            ":author": self.agent,
            ":author_basis": self.agent_basis,
            ":chain_op_type": ChainOpType::RegisterAgentActivity,
            ":warrant_op_type": WarrantOpType::ChainIntegrityWarrant,
            ":valid_status": ValidationStatus::Valid,
        };

        params.to_vec()
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

            match op_type {
                DhtOpType::Chain(_) => {
                    let hash: ActionHash = row.get("hash")?;
                    from_blob::<SignedAction>(row.get("action_blob")?).map(|action| {
                        let action = ActionHashed::with_pre_hashed(action.into_data(), hash);
                        let item = if integrated.is_some() {
                            Item::Integrated(action)
                        } else {
                            Item::Pending(action)
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

    fn fold(&self, state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        fold(state, data)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        render(state, self.agent.clone(), &self.filter, &self.options)
    }
}
