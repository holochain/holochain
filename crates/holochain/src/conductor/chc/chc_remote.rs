//! Defines a client for use with a remote HTTP-based CHC.
//!
//! **NOTE** this API is not set in stone. Do not design a CHC against this API yet,
//! as it will change!

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use ::bytes::Bytes;
use holo_hash::{ActionHash, EntryHash};
use holochain_types::{
    chc::{ChainHeadCoordinator, ChcError, ChcResult},
    prelude::{AddRecordsRequest, EncryptedEntry, GetRecordsRequest},
};
use holochain_zome_types::prelude::*;
use reqwest::Url;

/// An HTTP client which can talk to a remote CHC implementation
pub struct ChcRemote {
    client: ChcRemoteClient,
}

#[async_trait::async_trait]
impl ChainHeadCoordinator for ChcRemote {
    type Item = SignedActionHashed;

    async fn add_records(&self, request: AddRecordsRequest) -> ChcResult<()> {
        let body = serde_json::to_string(&request)
            .map(|json| json.into_bytes())
            .map_err(|e| SerializedBytesError::Serialize(e.to_string()))?;
        let response = self.client.post("/add_records", body).await?;
        todo!("parse and handle response");
    }

    async fn get_record_data(
        &self,
        request: GetRecordsRequest,
    ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>> {
        let body = serde_json::to_string(&request)
            .map(|json| json.into_bytes())
            .map_err(|e| SerializedBytesError::Serialize(e.to_string()))?;
        let response = self.client.post("/get_record_data", body).await?;
        todo!("parse and handle response");
    }
}

impl ChcRemote {
    /// Constructor
    pub fn new(url: Url, cell_id: &CellId) -> Self {
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
