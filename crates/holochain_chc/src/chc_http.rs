//! Defines a client for use with a remote HTTP-based CHC.
//!
//! The server must implement the following endpoints:
//!
//! ## `POST /add_records/{dna_hash}/{agent_pubkey}`
//!
//! Adds a list of records to the CHC.
//!
//! The CHC state will only be altered if a 200 status code is returned, which requires that:
//! - the new records are valid
//! - the signature matches the agent pubkey
//!
//! If the new records would cause a fork of the CHC chain but are otherwise valid, a 409 status code is returned
//! along with the sequence number and hash of the fork point. This code indicates to the client that the local
//! state should be synchronized with the CHC state before attempting to add the records again.
//! (by calling `get_record_data` and then "grafting" the records onto the local chain).
//!
//! If there is some other problem with the input record data which prevents it from being added to the CHC state,
//! e.g. the new records themselves do not constitute a valid hash chain, or the signature does not match,
//! a 498 status code may be returned to indicate that the input is bad and must be fixed.
//!
//! Any other error code can be returned to indicate a server error.
//!
//! Body: msgpack-encoded [`AddRecordsRequest`]
//! Response:
//! - 200: (no data)
//! - 409: msgpack-encoded `(u32, ActionHash)` (seq number and hash of fork point)
//! - 498: msgpack-encoded `u32` (seq number of last record in the CHC chain)
//! - other: error message as plaintext string
//!
//! ## `POST /get_record_data/{dna_hash}/{agent_pubkey}`
//!
//! Returns CHC data starting from the record *after* the given hash.
//!
//! If the given hash is not present in CHC state, Error code 498 should be returned with no data.
//!
//! A nonce must be provided in the request body to prevent replay attacks. The nonce need
//! not be truly random, just unique.
//!
//! **NOTE**: the `EncryptedEntry` data is not currently encrypted. Encryption is a TODO!
//!
//! Body: msgpack-encoded [`GetRecordsRequest`]
//! Response:
//! - 200: msgpack-encoded `Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>`
//! - 498: (no data)
//! - other: error message as plaintext string
//!
//! ## Notes (for both endpoints)
//!
//! The `{dna_hash}` and `{agent_pubkey}` in the URL are base64-encoded in the standard way.
//! (See the `Display` impl for `DnaHash` and `AgentPubKey`.)
//!
//! The request body is serialized using [`holochain_serialized_bytes::encode`] and can be deserialized using any
//! msgpack decoder.
//!
//! Any msgpack-encoded response must be encoded in a way that can deserialized by [`holochain_serialized_bytes::decode`].
//! Most standard msgpack encoders should work just fine for the return types being used here.
//!

use std::sync::Arc;

use super::ChainHeadCoordinatorExt;
use super::*;
use holochain_keystore::MetaLairClient;
use url::Url;

/// An HTTP client which can talk to a remote CHC implementation
pub struct ChcHttp {
    client: ChcHttpClient,
    keystore: MetaLairClient,
    agent: AgentPubKey,
}

#[async_trait::async_trait]
impl ChainHeadCoordinator for ChcHttp {
    type Item = SignedActionHashed;

    async fn add_records_request(&self, request: AddRecordsRequest) -> ChcResult<()> {
        let response: reqwest::Response = self.client.post("add_records", &request).await?;
        let status = response.status().as_u16();
        match status {
            200 => Ok(()),
            409 => {
                let bytes = response.bytes().await.map_err(extract_string)?;
                let (seq, hash): (u32, ActionHash) = holochain_serialized_bytes::decode(&bytes)?;
                Err(ChcError::InvalidChain(seq, hash))
            }
            498 => {
                let bytes = response.bytes().await.map_err(extract_string)?;
                let seq: u32 = holochain_serialized_bytes::decode(&bytes)?;
                Err(ChcError::NoRecordsAdded(seq))
            }
            code => {
                let msg = response.text().await.map_err(extract_string)?;
                Err(ChcError::Other(format!("code: {code}, msg: {msg}")))
            }
        }
    }

    async fn get_record_data_request(
        &self,
        request: GetRecordsRequest,
    ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>> {
        let response = self.client.post("get_record_data", &request).await?;
        let status = response.status().as_u16();
        match status {
            200 => {
                let bytes = response.bytes().await.map_err(extract_string)?;
                Ok(holochain_serialized_bytes::decode(&bytes)?)
            }
            498 => {
                // The since_hash was not found in the CHC,
                // so we can interpret this as an empty list of records.
                Ok(vec![])
            }
            code => {
                let msg = response.text().await.map_err(extract_string)?;
                Err(ChcError::Other(format!("code: {code}, msg: {msg}")))
            }
        }
    }
}

impl ChainHeadCoordinatorExt for ChcHttp {
    fn signing_info(&self) -> (MetaLairClient, AgentPubKey) {
        (self.keystore.clone(), self.agent.clone())
    }
}

impl ChcHttp {
    /// Constructor
    pub fn new(base_url: Url, keystore: MetaLairClient, cell_id: &CellId) -> Self {
        let client = ChcHttpClient {
            base_url: base_url
                .join(&format!(
                    "{}/{}/",
                    cell_id.dna_hash(),
                    cell_id.agent_pubkey()
                ))
                .expect("invalid URL"),
        };
        Self {
            client,
            keystore,
            agent: cell_id.agent_pubkey().clone(),
        }
    }
}

/// Client for a single CHC server
pub struct ChcHttpClient {
    base_url: url::Url,
}

impl ChcHttpClient {
    fn url(&self, path: &str) -> String {
        assert!(!path.starts_with('/'));
        self.base_url.join(path).expect("invalid URL").to_string()
    }

    async fn post<T>(&self, path: &str, body: &T) -> ChcResult<reqwest::Response>
    where
        T: serde::Serialize + std::fmt::Debug,
    {
        let client = reqwest::Client::new();
        let url = self.url(path);
        let body = holochain_serialized_bytes::encode(body)?;
        let res: reqwest::Response = client
            .post(url.clone())
            .body(body)
            .send()
            .await
            .map_err(extract_string)?;
        Ok(res)
    }
}

fn extract_string(e: reqwest::Error) -> ChcError {
    ChcError::ServiceUnreachable(e.to_string())
}

#[cfg(test)]
mod tests {

    use super::*;
    use holochain_types::test_utils::valid_arbitrary_chain;
    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "this test requires a remote service, so it should only be run manually"]
    async fn test_add_records_remote() {
        let keystore = holochain_keystore::test_keystore();
        let agent = fake_agent_pubkey_1();
        let cell_id = CellId::new(::fixt::fixt!(DnaHash), agent.clone());
        let chc = Arc::new(ChcHttp::new(
            url::Url::parse("http://127.0.0.1:40845/").unwrap(),
            // url::Url::parse("https://chc.dev.holotest.net/v1/").unwrap(),
            keystore.clone(),
            &cell_id,
        ));

        let mut g = random_generator();

        let chain = valid_arbitrary_chain(&mut g, keystore, agent, 20).await;

        let t0 = &chain[0..3];
        let t1 = &chain[3..6];
        let t2 = &chain[6..9];
        let t11 = &chain[11..=11];

        let hash = |i: usize| chain[i].action_address().clone();

        // dbg!(t0
        //     .iter()
        //     .map(|r| (r.action_address(), r.action().prev_action()))
        //     .collect::<Vec<_>>());

        // dbg!(&t0, &t1, &t2);

        chc.clone()
            .add_records(t0.to_vec())
            .await
            .map_err(|e| e.to_string()[..1024.min(e.to_string().len())].to_string())
            .unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(2));

        chc.clone().add_records(t1.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        // last_hash doesn't match
        assert!(chc.clone().add_records(t0.to_vec()).await.is_err());
        assert!(chc.clone().add_records(t1.to_vec()).await.is_err());
        assert!(chc.clone().add_records(t11.to_vec()).await.is_err());
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        chc.clone().add_records(t2.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(8));

        assert_eq!(
            chc.clone().get_record_data(None).await.unwrap(),
            &chain[0..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(0))).await.unwrap(),
            &chain[1..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(3))).await.unwrap(),
            &chain[4..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(7))).await.unwrap(),
            &chain[8..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(8))).await.unwrap(),
            &[]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(9))).await.unwrap(),
            &[]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(13))).await.unwrap(),
            &[]
        );
    }
}
