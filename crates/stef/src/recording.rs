use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

use crate::*;

/// Shared access to FetchPoolState
#[derive(Clone, derive_more::Deref, derive_more::DerefMut, derivative::Derivative)]
#[derivative(Debug)]
pub struct RecordActions<S> {
    #[deref]
    #[deref_mut]
    state: S,
    #[derivative(Debug = "ignore")]
    cassette: Arc<dyn Cassette<S> + Send + Sync>,
}

impl<S> State<'static> for RecordActions<S>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
{
    type Action = S::Action;
    type Effect = S::Effect;

    fn transition(&mut self, action: Self::Action) -> Self::Effect {
        self.cassette.record_action(&action).unwrap();
        self.state.transition(action)
    }
}

impl<S> RecordActions<S>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
{
    pub fn new(cassette: Option<impl Cassette<S> + Send + Sync + 'static>, state: S) -> Self {
        let cassette: Arc<dyn Cassette<S> + Send + Sync> = if let Some(c) = cassette {
            Arc::new(c)
        } else {
            Arc::new(())
        };
        cassette.initialize().unwrap();
        Self { cassette, state }
    }
}

#[test]
fn action_recording_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("actions.stef");
    let cassette = FileCassette::<()>::from(path.clone());
    let mut rec = RecordActions::new(Some(cassette), ());
    rec.transition(());
    rec.transition(());
    rec.transition(());
    drop(rec);
    
    let cassette = FileCassette::<()>::from(path.clone());
    let actions: Vec<()> = cassette.retrieve_actions().unwrap();
    assert_eq!(actions, vec![(), (), ()]);
}
