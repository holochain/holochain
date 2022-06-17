use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::Judged;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetRecordDetailsQuery(ActionHash, Option<Arc<AgentPubKey>>);

impl GetRecordDetailsQuery {
    pub fn new(hash: ActionHash) -> Self {
        Self(hash, None)
    }
}

#[derive(Debug)]
pub struct State {
    action: Option<SignedActionHashed>,
    rejected_action: Option<SignedActionHashed>,
    deletes: HashSet<SignedActionHashed>,
    updates: HashSet<SignedActionHashed>,
}

impl Query for GetRecordDetailsQuery {
    type Item = Judged<SignedActionHashed>;
    type State = State;
    type Output = Option<RecordDetails>;

    fn query(&self) -> String {
        "
        SELECT Action.blob AS action_blob, DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Action On DhtOp.action_hash = Action.hash
        WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
        AND DhtOp.basis_hash = :action_hash
        AND DhtOp.when_integrated IS NOT NULL
        AND DhtOp.validation_status IS NOT NULL
        "
        .into()
    }
    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create_type": DhtOpType::StoreRecord,
            ":delete_type": DhtOpType::RegisterDeletedBy,
            ":update_type": DhtOpType::RegisterUpdatedRecord,
            ":action_hash": self.0,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = |row: &Row| {
            let action =
                from_blob::<SignedAction>(row.get(row.as_ref().column_index("action_blob")?)?)?;
            let SignedAction(action, signature) = action;
            let action = ActionHashed::from_content_sync(action);
            let shh = SignedActionHashed::with_presigned(action, signature);
            let status = row.get(row.as_ref().column_index("status")?)?;
            let r = Judged::new(shh, status);
            Ok(r)
        };
        Arc::new(f)
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let action_filter = self.0.clone();
        let f = move |action: &QueryData<Self>| {
            let action = &action;
            if *action.action_address() == action_filter {
                true
            } else {
                match action.action() {
                    Action::Delete(Delete {
                        deletes_address, ..
                    }) => *deletes_address == action_filter,
                    Action::Update(Update {
                        original_action_address,
                        ..
                    }) => *original_action_address == action_filter,
                    _ => false,
                }
            }
        };
        Box::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(State {
            action: Default::default(),
            rejected_action: Default::default(),
            deletes: Default::default(),
            updates: Default::default(),
        })
    }

    fn fold(&self, mut state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        let (shh, validation_status) = item.into();
        if *shh.as_hash() == self.0 {
            if state.action.is_none() && state.rejected_action.is_none() {
                match validation_status {
                    Some(ValidationStatus::Valid) => {
                        state.action = Some(shh);
                    }
                    Some(ValidationStatus::Rejected) => {
                        state.rejected_action = Some(shh);
                    }
                    _ => (),
                }
            }
        } else {
            match shh.action() {
                Action::Update(Update {
                    original_action_address,
                    ..
                }) if *original_action_address == self.0 => {
                    state.updates.insert(shh);
                }
                Action::Delete(Delete {
                    deletes_address, ..
                }) if *deletes_address == self.0 => {
                    state.deletes.insert(shh);
                }
                _ => (),
            }
        }

        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let State {
            action,
            rejected_action,
            deletes,
            updates,
        } = state;

        let (action, validation_status) = match (action, rejected_action) {
            (None, None) => return Ok(None),
            (None, Some(h)) => (h, ValidationStatus::Rejected),
            (Some(h), None) => (h, ValidationStatus::Valid),
            (Some(_), Some(h)) => {
                // TODO: this is a conflict between multiple sources and
                // needs to be handled.
                (h, ValidationStatus::Rejected)
            }
        };

        let mut entry = None;
        if let Some(entry_hash) = action.action().entry_hash() {
            let author = self
                .1
                .as_ref()
                .map(|a| a.as_ref())
                .filter(|a| *a == action.action().author());
            entry = stores.get_public_or_authored_entry(entry_hash, author)?;
        }
        let record = Record::new(action, entry);
        let details = RecordDetails {
            record,
            validation_status,
            deletes: deletes.into_iter().collect(),
            updates: updates.into_iter().collect(),
        };
        Ok(Some(details))
    }
}

impl PrivateDataQuery for GetRecordDetailsQuery {
    type Hash = ActionHash;

    fn with_private_data_access(hash: Self::Hash, author: Arc<AgentPubKey>) -> Self {
        Self(hash, Some(author))
    }

    fn without_private_data_access(hash: Self::Hash) -> Self {
        Self::new(hash)
    }
}
