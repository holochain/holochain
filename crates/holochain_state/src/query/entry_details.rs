use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::DhtOpError;
use holochain_types::prelude::Judged;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetEntryDetailsQuery(EntryHash, Option<Arc<AgentPubKey>>);

impl GetEntryDetailsQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash, None)
    }
}

pub struct State {
    actions: HashSet<SignedActionHashed>,
    rejected_actions: HashSet<SignedActionHashed>,
    deletes: HashMap<ActionHash, SignedActionHashed>,
    updates: HashSet<SignedActionHashed>,
}

impl Query for GetEntryDetailsQuery {
    type Item = Judged<SignedActionHashed>;
    type State = State;
    type Output = Option<EntryDetails>;

    fn query(&self) -> String {
        "
        SELECT Action.blob AS action_blob, DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Action On DhtOp.action_hash = Action.hash
        WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
        AND DhtOp.basis_hash = :entry_hash
        AND DhtOp.when_integrated IS NOT NULL
        AND DhtOp.validation_status IS NOT NULL
        AND (Action.private_entry = 0 OR Action.private_entry IS NULL OR Action.author = :author)
        "
        .into()
    }
    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create_type": DhtOpType::StoreEntry,
            ":delete_type": DhtOpType::RegisterDeletedEntryAction,
            ":update_type": DhtOpType::RegisterUpdatedContent,
            ":entry_hash": self.0,
            ":author": self.1,
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
        let entry_filter = self.0.clone();
        let f = move |action: &QueryData<Self>| {
            let action = &action;
            match action.action() {
                Action::Create(Create { entry_hash, .. })
                | Action::Update(Update { entry_hash, .. })
                    if *entry_hash == entry_filter =>
                {
                    true
                }
                Action::Update(Update {
                    original_entry_address,
                    ..
                }) => *original_entry_address == entry_filter,
                Action::Delete(Delete {
                    deletes_entry_address,
                    ..
                }) => *deletes_entry_address == entry_filter,
                _ => false,
            }
        };
        Box::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(State {
            actions: Default::default(),
            rejected_actions: Default::default(),
            deletes: Default::default(),
            updates: Default::default(),
        })
    }

    fn fold(&self, mut state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        let (shh, validation_status) = item.into();
        let add_action = |state: &mut State, shh| match validation_status {
            Some(ValidationStatus::Valid) => {
                state.actions.insert(shh);
            }
            Some(ValidationStatus::Rejected) => {
                state.rejected_actions.insert(shh);
            }
            _ => (),
        };
        match shh.action() {
            Action::Create(_) => add_action(&mut state, shh),
            Action::Update(update) => {
                if update.original_entry_address == self.0 && update.entry_hash == self.0 {
                    state.updates.insert(shh.clone());
                    add_action(&mut state, shh);
                } else if update.entry_hash == self.0 {
                    add_action(&mut state, shh);
                } else if update.original_entry_address == self.0 {
                    state.updates.insert(shh.clone());
                }
            }
            Action::Delete(delete) => {
                let hash = delete.deletes_address.clone();
                state.deletes.insert(hash, shh.clone());
            }
            _ => {
                return Err(StateQueryError::UnexpectedAction(
                    shh.action().action_type(),
                ))
            }
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        // Choose an arbitrary action.
        // TODO: Is it sound to us a rejected action here?
        let action = state
            .actions
            .iter()
            .chain(state.rejected_actions.iter())
            .next();
        match action {
            Some(action) => {
                let entry_hash = action
                    .action()
                    .entry_hash()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.action().clone()))?;
                let author = self.1.as_ref().map(|a| a.as_ref());
                let details = stores
                    .get_public_or_authored_entry(entry_hash, author)?
                    .map(|entry| {
                        let entry_dht_status = compute_entry_status(&state);
                        EntryDetails {
                            entry,
                            actions: state.actions.into_iter().collect(),
                            rejected_actions: state.rejected_actions.into_iter().collect(),
                            deletes: state.deletes.into_values().collect(),
                            updates: state.updates.into_iter().collect(),
                            entry_dht_status,
                        }
                    });
                Ok(details)
            }
            None => Ok(None),
        }
    }
}

fn compute_entry_status(state: &State) -> EntryDhtStatus {
    let live_actions = state
        .actions
        .iter()
        .filter(|h| !state.deletes.contains_key(h.action_address()))
        .count();
    if live_actions > 0 {
        EntryDhtStatus::Live
    } else {
        EntryDhtStatus::Dead
    }
}

impl PrivateDataQuery for GetEntryDetailsQuery {
    type Hash = EntryHash;

    fn with_private_data_access(hash: Self::Hash, author: Arc<AgentPubKey>) -> Self {
        Self(hash, Some(author))
    }

    fn without_private_data_access(hash: Self::Hash) -> Self {
        Self::new(hash)
    }
}
