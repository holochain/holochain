use std::collections::{HashMap, HashSet};

use ::bytes::Bytes;
use holo_hash::{ActionHash, EntryHash};
use holochain_serialized_bytes::{decode, encode};
use holochain_types::chc::{ChainHeadCoordinator, ChcError, ChcResult};
use holochain_zome_types::prelude::*;
use reqwest::Url;

/// An HTTP client which can talk to a remote CHC implementation
pub struct ChcRemote {
    actions: ChcRemoteClient,
    entries: ChcRemoteClient,
}

#[async_trait::async_trait]
impl ChainHeadCoordinator for ChcRemote {
    type Item = SignedActionHashed;

    async fn head(&self) -> ChcResult<Option<ActionHash>> {
        let response = self.actions.get("/head").await?;
        Ok(decode(&response)?)
    }

    async fn add_actions(&self, actions: Vec<Self::Item>) -> ChcResult<()> {
        let body = encode(&actions)?;
        let _response = self.actions.post("/add_actions", body).await?;
        Ok(())
    }

    async fn add_entries(&self, entries: Vec<EntryHashed>) -> ChcResult<()> {
        let body = encode(&entries)?;
        let _response = self.entries.post("/add_entries", body).await?;
        Ok(())
    }

    async fn get_actions_since_hash(&self, hash: Option<ActionHash>) -> ChcResult<Vec<Self::Item>> {
        let body = encode(&hash)?;
        let response = self.actions.post("/get_actions_since_hash", body).await?;
        Ok(decode(&response)?)
    }

    async fn get_entries(
        &self,
        _hashes: HashSet<&EntryHash>,
    ) -> ChcResult<HashMap<EntryHash, Entry>> {
        todo!()
    }
}

impl ChcRemote {
    /// Constructor
    pub fn new(_namespace: &str, _cell_id: &CellId) -> Self {
        todo!("Implement remote CHC client")
    }
}

/// Client for a single CHC server
pub struct ChcRemoteClient {
    base_url: url::Url,
}

impl ChcRemoteClient {
    fn url(&self, path: &str) -> Url {
        assert!(path.chars().nth(0) == Some('/'));
        Url::parse(&format!("{}{}", self.base_url, path)).expect("invalid URL")
    }

    async fn get(&self, path: &str) -> ChcResult<Bytes> {
        let bytes = reqwest::get(self.url(path))
            .await
            .map_err(extract_string)?
            .bytes()
            .await
            .map_err(extract_string)?;
        Ok(bytes)
    }

    async fn post(&self, path: &str, body: Vec<u8>) -> ChcResult<Bytes> {
        let client = reqwest::Client::new();
        let response = client
            .post(self.url(path))
            .body(body)
            .send()
            .await
            .map_err(extract_string)?;
        Ok(response.bytes().await.map_err(extract_string)?)
    }
}

fn extract_string(e: reqwest::Error) -> ChcError {
    ChcError::ServiceUnreachable(e.to_string())
}
