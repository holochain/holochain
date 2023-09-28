use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;

use crate::*;

/// Shared access to FetchPoolState
#[derive(Clone, Debug, derive_more::Deref, derive_more::DerefMut)]
pub struct RecordActions<S> {
    #[deref]
    #[deref_mut]
    state: S,
    storage: Option<Arc<PathBuf>>,
}

impl<S> State<'static> for RecordActions<S>
where
    S: State<'static>,
    S::Action: Serialize,
{
    type Action = S::Action;
    type Effect = S::Effect;

    fn transition(&mut self, action: Self::Action) -> Self::Effect {
        if let Some(path) = self.storage.as_ref() {
            let bytes = rmp_serde::to_vec(&action).unwrap();
            let mut f = File::options().append(true).open(&**path).unwrap();
            let len = bytes.len() as u32;
            f.write(&len.to_le_bytes()).unwrap();
            f.write(&bytes).unwrap();
        }
        self.state.transition(action)
    }
}

impl<S> RecordActions<S>
where
    S: State<'static>,
    S::Action: serde::de::DeserializeOwned,
{
    pub fn new(storage: Option<PathBuf>, state: S) -> Self {
        if let Some(path) = storage.as_ref() {
            File::options()
                .write(true)
                .create_new(true)
                .open(path)
                .expect("specified the same file twice in RecordActions");
        }
        Self {
            storage: storage.map(Arc::new),
            state,
        }
    }

    pub fn read_actions_from_file(path: impl AsRef<Path>) -> std::io::Result<Vec<S::Action>> {
        let mut f = File::open(path)?;
        let mut lbuf = [0; 4];
        let mut abuf = Vec::new();
        let mut actions = Vec::new();
        loop {
            match f.read(&mut lbuf) {
                Ok(0) => return Ok(actions),
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        {
                            return Ok(actions);
                        }
                    } else {
                        return Err(e);
                    }
                }
                Ok(_) => {
                    let len = u32::from_le_bytes(lbuf);
                    abuf.resize(len as usize, 0);
                    f.read(&mut abuf)?;
                    actions.push(rmp_serde::from_slice(&abuf).unwrap());
                }
            }
        }
    }
}

#[test]
fn action_recording_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("actions.stef");
    let mut rec = RecordActions::new(Some(path.clone()), ());
    rec.transition(());
    rec.transition(());
    rec.transition(());
    let actions: Vec<()> = RecordActions::<()>::read_actions_from_file(&path).unwrap();
    assert_eq!(actions, vec![(), (), ()]);
}
