use crate::authority::get_agent_activity_query::actions::GetAgentActivityActionsQuery;
use crate::authority::get_agent_activity_query::{fold, render, Item, State};
use holo_hash::{ActionHash, AgentPubKey, WarrantHash};
use holochain_p2p::dht::op::Timestamp;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::Row;
use holochain_state::prelude::{from_blob, ActionHashed, Params, Query, StateQueryResult, Store};
use holochain_state::query::QueryData;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::judged::Judged;
use holochain_zome_types::prelude::{
    ChainQueryFilter, SignedAction, SignedWarrant, ValidationStatus,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GetAgentActivityHashesQuery {
    actions_query: GetAgentActivityActionsQuery,
}

impl GetAgentActivityHashesQuery {
    pub fn new(agent: AgentPubKey, filter: ChainQueryFilter, options: GetActivityOptions) -> Self {
        Self {
            actions_query: GetAgentActivityActionsQuery::new(agent, filter, options),
        }
    }
}

impl Query for GetAgentActivityHashesQuery {
    type State = State<ActionHashed>;
    type Item = Judged<Item<ActionHashed>>;
    type Output = AgentActivityResponse;

    fn query(&self) -> String {
        self.actions_query.query()
    }

    fn params(&self) -> Vec<Params> {
        self.actions_query.params()
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
        render(
            state,
            self.actions_query.agent.clone(),
            &self.actions_query.filter,
            &self.actions_query.options,
        )
    }
}
