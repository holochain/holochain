use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

use crate::*;

/// Shared access to FetchPoolState
#[derive(Clone, Debug, derive_more::Deref)]
pub struct RecordActions<S, C = FileCassette<S>> {
    #[deref]
    state: S,
    cassette: Arc<C>,
}

impl<S, R> State<'static> for RecordActions<S, R>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    R: Cassette<S>,
{
    type Action = S::Action;
    type Effect = S::Effect;

    fn transition(&mut self, action: Self::Action) -> Self::Effect {
        self.cassette.record_action(&action).unwrap();
        self.state.transition(action)
    }
}

impl<S, C> RecordActions<S, C>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    C: Cassette<S>,
{
    pub fn new(cassette: C, state: S) -> Self {
        cassette.initialize().unwrap();
        Self {
            cassette: Arc::new(cassette),
            state,
        }
    }
}

#[test]
fn action_recording_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("actions.stef");
    let mut rec = RecordActions::new(FileCassette::from(path.clone()), ());
    rec.transition(());
    rec.transition(());
    rec.transition(());
    let actions: Vec<()> = FileCassette::<()>::from(path.clone())
        .retrieve_actions()
        .unwrap();
    assert_eq!(actions, vec![(), (), ()]);
}
