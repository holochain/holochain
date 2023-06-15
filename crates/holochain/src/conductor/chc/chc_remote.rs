//! Defines a client for use with a remote HTTP-based CHC.

use std::sync::Arc;

use holo_hash::{ActionHash, AgentPubKey};
use holochain_keystore::MetaLairClient;
use holochain_types::{
    chc::{ChainHeadCoordinator, ChcError, ChcResult},
    prelude::{AddRecordsRequest, ChainHeadCoordinatorExt, EncryptedEntry, GetRecordsRequest},
};
use holochain_zome_types::prelude::*;
use url::Url;
use url2::Url2;

/// An HTTP client which can talk to a remote CHC implementation
pub struct ChcRemote {
    client: ChcRemoteClient,
    keystore: MetaLairClient,
    agent: AgentPubKey,
}

#[async_trait::async_trait]
impl ChainHeadCoordinator for ChcRemote {
    type Item = SignedActionHashed;

    async fn add_records_request(&self, request: AddRecordsRequest) -> ChcResult<()> {
        let body = serde_json::to_string(&request)
            .map(|json| json.into_bytes())
            .map_err(|e| SerializedBytesError::Serialize(e.to_string()))?;
        let response: reqwest::Response = self.client.post("add_records", body).await?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await.map_err(extract_string)?;
        dbg!(std::str::from_utf8(&bytes).unwrap());
        dbg!(match status {
            200 => Ok(()),
            409 => {
                let (seq, hash): (u32, ActionHash) = serde_json::from_slice(&bytes)?;
                Err(ChcError::InvalidChain(seq, hash, "".to_string()))
            }
            498 => {
                let msg: String = serde_json::from_slice(&bytes)?;
                Err(ChcError::NoRecordsAdded(msg))
            }
            code => {
                let msg: String = serde_json::from_slice(&bytes)?;
                Err(ChcError::Other(format!("code: {code}, msg: {msg}")))
            }
        })
    }

    async fn get_record_data_request(
        &self,
        request: GetRecordsRequest,
    ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>> {
        let body = serde_json::to_string(&request)
            .map(|json| json.into_bytes())
            .map_err(|e| SerializedBytesError::Serialize(e.to_string()))?;
        let response = self.client.post("get_record_data", body).await?;
        match response.status() {
            _ => todo!(),
        }
    }
}

impl ChainHeadCoordinatorExt for ChcRemote {
    fn signing_info(&self) -> (MetaLairClient, holo_hash::AgentPubKey) {
        (self.keystore.clone(), self.agent.clone())
    }
}

impl ChcRemote {
    /// Constructor
    pub fn new(base_url: Url, keystore: MetaLairClient, cell_id: &CellId) -> Self {
        let client = ChcRemoteClient {
            base_url: dbg!(base_url
                .join(&format!(
                    "{}/{}/",
                    cell_id.dna_hash(),
                    cell_id.agent_pubkey()
                ))
                .expect("invalid URL")),
        };
        Self {
            client,
            keystore,
            agent: cell_id.agent_pubkey().clone(),
        }
    }
}

/// Client for a single CHC server
pub struct ChcRemoteClient {
    base_url: url::Url,
}

impl ChcRemoteClient {
    fn url(&self, path: &str) -> String {
        assert!(path.chars().nth(0) != Some('/'));
        dbg!(self.base_url.join(path).expect("invalid URL").to_string())
    }

    // async fn get(&self, path: &str) -> ChcResult<reqwest::Response> {
    //     reqwest::get(self.url(path)).await.map_err(extract_string)
    // }

    async fn post(&self, path: &str, body: Vec<u8>) -> ChcResult<reqwest::Response> {
        let client = reqwest::Client::new();
        let url = self.url(path);
        dbg!(&url);
        client
            .post(url)
            .body(body)
            .send()
            .await
            .map_err(extract_string)
    }
}

fn extract_string(e: reqwest::Error) -> ChcError {
    ChcError::ServiceUnreachable(e.to_string())
}
