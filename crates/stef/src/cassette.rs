use std::{
    borrow::Borrow,
    fs::File,
    io::{Read, Write},
    marker::PhantomData,
    path::PathBuf,
};

use kitsune_p2p_timestamp::Timestamp;
use serde::{de::DeserializeOwned, Serialize};

use crate::*;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CassetteAction<A, B: Borrow<A> = A> {
    pub action: B,
    pub timestamp: Timestamp,
    phantom: PhantomData<A>,
}

impl<A, B: Borrow<A>> CassetteAction<A, B> {
    pub fn new(action: B) -> Self {
        Self {
            action,
            timestamp: Timestamp::now(),
            phantom: PhantomData,
        }
    }
}

pub trait Cassette<S: State<'static>> {
    fn initialize(&self) -> anyhow::Result<()>;

    fn record_action(&self, action: &S::Action) -> anyhow::Result<()>;

    // TODO: use fallible_iterator for lazy retrieval
    fn retrieve_actions(&self) -> anyhow::Result<Vec<CassetteAction<S::Action>>>;

    fn playback_actions(&self, state: &mut S) -> anyhow::Result<Vec<S::Effect>> {
        Ok(self
            .retrieve_actions()?
            .into_iter()
            .map(|action| state.transition(action.action))
            .collect())
    }
}

impl<S: State<'static>> Cassette<S> for () {
    fn initialize(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn record_action(&self, _: &S::Action) -> anyhow::Result<()> {
        Ok(())
    }

    fn retrieve_actions(&self) -> anyhow::Result<Vec<CassetteAction<S::Action>>> {
        unimplemented!("The unit ActionRecorder `()` can't record or playback actions!")
    }
}

pub struct FileCassette<S, E = RmpEncoder>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder,
{
    path: PathBuf,
    encoder: E,
    erase_existing: bool,
    state: PhantomData<(S, E)>,
}

impl<S, E> From<PathBuf> for FileCassette<S, E>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder,
{
    fn from(path: PathBuf) -> Self {
        Self::new(path, Default::default(), true)
    }
}

impl<S, E> FileCassette<S, E>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder,
{
    pub fn new(path: PathBuf, encoder: E, erase_existing: bool) -> Self {
        Self {
            path,
            encoder,
            erase_existing,
            state: PhantomData,
        }
    }
}

impl<S, E> Cassette<S> for FileCassette<S, E>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    for<'a> &'a S::Action: Serialize,
    E: Encoder,
{
    fn initialize(&self) -> anyhow::Result<()> {
        let mut f = File::options();
        f.write(true);
        if self.erase_existing {
            if let Err(err) = f.truncate(true).open(&self.path) {
                tracing::error!("Error opening stef cassette: {:?}", err);
            }
        } else {
            f.create_new(true).open(&self.path)?;
        };
        Ok(())
    }

    fn record_action(&self, action: &S::Action) -> anyhow::Result<()> {
        let action: CassetteAction<S::Action, &<S as state::State<'static>>::Action> =
            CassetteAction::new(action);
        let bytes = self.encoder.encode(&action)?;
        let mut f = File::options().append(true).open(&self.path)?;
        let len = bytes.len() as u32;
        f.write_all(&len.to_le_bytes())?;
        f.write_all(&bytes)?;
        Ok(())
    }

    fn retrieve_actions(&self) -> anyhow::Result<Vec<CassetteAction<S::Action>>> {
        let mut f = File::open(&self.path)?;
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
                        return Err(e.into());
                    }
                }
                Ok(_) => {
                    let len = u32::from_le_bytes(lbuf);
                    abuf.resize(len as usize, 0);
                    f.read_exact(&mut abuf)?;
                    actions.push(self.encoder.decode(&abuf).unwrap());
                }
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct MemoryCassette<S: State<'static>>
where
    S::Action: Clone + Serialize + DeserializeOwned,
{
    actions: Share<Vec<CassetteAction<S::Action, S::Action>>>,
    state: PhantomData<S>,
}

impl<S: State<'static>> MemoryCassette<S>
where
    S::Action: Clone + Serialize + DeserializeOwned,
{
    pub fn new() -> Self {
        Self {
            actions: Default::default(),
            state: Default::default(),
        }
    }
}

impl<S: State<'static>> Cassette<S> for MemoryCassette<S>
where
    S::Action: Clone + Serialize + DeserializeOwned,
{
    fn initialize(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn record_action(&self, action: &S::Action) -> anyhow::Result<()> {
        let action = CassetteAction::new(action.clone());
        self.actions.write(|aa| aa.push(action));
        Ok(())
    }

    fn retrieve_actions(&self) -> anyhow::Result<Vec<CassetteAction<S::Action>>> {
        Ok(self.actions.read(|aa| aa.clone()))
    }
}
