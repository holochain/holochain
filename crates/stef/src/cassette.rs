use std::{
    fs::File,
    io::{Read, Write},
    marker::PhantomData,
    path::PathBuf,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::*;

pub trait Cassette<S: State<'static>> {
    fn initialize(&self) -> anyhow::Result<()>;

    fn record_action(&self, action: &S::Action) -> anyhow::Result<()>;

    // TODO: use fallible_iterator for lazy retrieval
    fn retrieve_actions(&self) -> anyhow::Result<Vec<S::Action>>;

    fn playback_actions(&self, state: &mut S) -> anyhow::Result<Vec<S::Effect>> {
        Ok(self
            .retrieve_actions()?
            .into_iter()
            .map(|action| state.transition(action))
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

    fn retrieve_actions(&self) -> anyhow::Result<Vec<S::Action>> {
        unimplemented!("The unit ActionRecorder `()` can't record or playback actions!")
    }
}

pub struct FileCassette<S, E = RmpEncoder>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder<S::Action>,
{
    path: PathBuf,
    encoder: E,
    state: PhantomData<(S, E)>,
}

impl<S, E> From<PathBuf> for FileCassette<S, E>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder<S::Action>,
{
    fn from(path: PathBuf) -> Self {
        Self::new(path, Default::default())
    }
}

impl<S, E> FileCassette<S, E>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder<S::Action>,
{
    pub fn new(path: PathBuf, encoder: E) -> Self {
        Self {
            path,
            encoder,
            state: PhantomData,
        }
    }
}

impl<S, E> Cassette<S> for FileCassette<S, E>
where
    S: State<'static>,
    S::Action: Serialize + DeserializeOwned,
    E: Encoder<S::Action>,
{
    fn initialize(&self) -> anyhow::Result<()> {
        File::options()
            .write(true)
            .create_new(true)
            .open(&self.path)?;
        Ok(())
    }

    fn record_action(&self, action: &S::Action) -> anyhow::Result<()> {
        let bytes = self.encoder.encode(action)?;
        let mut f = File::options().append(true).open(&self.path)?;
        let len = bytes.len() as u32;
        f.write_all(&len.to_le_bytes())?;
        f.write_all(&bytes)?;
        Ok(())
    }

    fn retrieve_actions(&self) -> anyhow::Result<Vec<S::Action>> {
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

#[derive(Clone)]
pub struct MemoryCassette<S: State<'static>> {
    actions: Share<Vec<S::Action>>,
    state: PhantomData<S>,
}

impl<S: State<'static>> MemoryCassette<S> {
    pub fn new() -> Self {
        Self {
            actions: Default::default(),
            state: Default::default(),
        }
    }
}

impl<S: State<'static>> Cassette<S> for MemoryCassette<S>
where
    S::Action: Serialize + DeserializeOwned + Clone,
{
    fn initialize(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn record_action(&self, action: &S::Action) -> anyhow::Result<()> {
        dbg!();
        self.actions.write(|aa| aa.push(action.clone()));
        Ok(())
    }

    fn retrieve_actions(&self) -> anyhow::Result<Vec<S::Action>> {
        Ok(self.actions.read(|aa| aa.clone()))
    }
}
