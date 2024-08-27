//! Defines the Chain Head Coordination API and an HTTP client for talking to a remote CHC server.
//!
//! A Chain Head Coordinator (CHC) is an external service which Holochain can communicate with.
//! A CHC is used in situations where it is desirable to run the same Holochain Cell (source chain)
//! on multiple different conductors. The CHC ensures that these multiple devices don't inadvertently
//! create a fork of the source chain by doing simultaneous uncoordinated writes.
//!
//! This crate introduces a [`ChainHeadCoordinator`] trait which defines the interface that Holochain
//! weaves into its logic to make use of a CHC service. Currently, the only Holochain-supported implementation
//! of this trait is [`ChcHttp`][chc_http::ChcHttp], which makes HTTP requests to a remote server and returns the responses.
//! There is also a [`ChcLocal`][chc_local::ChcLocal] reference implementation which is used for testing.
//! Other implementations can be written as needed, including alternate implementations of a HTTP client,
//! or also other kinds of clients using other protocols.
//!
//! Holochain specifies an optional `chc_url` field in its configuration which can point to an HTTP
//! server that implements the CHC interface.
//! See the [`chc_http`] module docs for specs on how to set up a remote CHC HTTP server that
//! Holochain can talk to using the provided [`ChcHttp`] implementation.
//!
//! The CHC trait contains two methods, and Holochain actually only uses one of them:
//! Every time Holochain is about to commit some records to a source chain, it first calls
//! [add_records_request][`ChainHeadCoordinator::add_records_request`]
//! to request that the CHC store those new records.
//! The CHC must be written so that it recognizes that the new records being added are a valid
//! extension of the existing chain of stored records, i.e. that the first record being pushed has a
//! [`Action::prev_action`] field which matches the hash of the last record already stored,
//! and additionally that the new records also form a valid hash chain (via `prev_action`).
//! If the new records are valid, they are added to CHC's chain, and an Ok result is returned.
//!
//! If these criteria for valid new records are not met, the CHC must return [`ChcError::InvalidChain`],
//! which lets the client know that the local chain is out of sync with CHC's version,
//! due to some other conductor having updated its own chain. In this case, the user (the driver
//! of the conductor) should call the other CHC method,
//! [get_record_data_request][`ChainHeadCoordinator::get_record_data_request`],
//! which will return the diff of records that CHC has that the local conductor does not.
//! Then, the Holochain admin method `GraftRecords` can be called to "stitch" these records onto the
//! existing chain, removing any fork if necessary.
//!
//! Note that currently, [get_record_data_request][`ChainHeadCoordinator::get_record_data_request`]
//! is never called directly by Holochain, but it might be in the future.
//!
//! Note also that when a CHC is used, the CHC is always considered the authoritative source of truth.
//! If a local conductor's state for whatever reason contradicts the CHC in any way, whether the local
//! chain contains different data altogether or is strictly ahead of the CHC, the CHC's version is always
//! considered correct and the local chain should always be modified to match the CHC's state.
//!

use std::{collections::HashMap, fmt::Debug, sync::Arc};

use futures::FutureExt;
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_nonce::Nonce256Bits;
use holochain_serialized_bytes::SerializedBytesError;
use holochain_types::prelude::*;
use must_future::MustBoxFuture;

use holochain_types::chain::ChainItem;

pub mod chc_http;
pub mod chc_local;

/// The API which a Chain Head Coordinator service must implement.
#[async_trait::async_trait]
pub trait ChainHeadCoordinator {
    /// The item which the chain is made of.
    type Item: ChainItem;

    /// Request that the CHC append these records to its chain.
    ///
    /// Whenever Holochain is about to commit something, this function will first be called.
    /// The CHC will do some integrity checks, which may fail.
    /// All signatures and hashes need to line up properly.
    /// If the records added would result in a fork, then a [`ChcError::InvalidChain`] will be returned
    /// along with the current chain top.
    // If there is an out-of-sync error, it will return a hash, designating the point of fork.
    async fn add_records_request(&self, request: AddRecordsRequest) -> ChcResult<()>;

    /// Get actions after (not including) the given hash.
    async fn get_record_data_request(
        &self,
        request: GetRecordsRequest,
    ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>>;
}

/// Add some convenience methods to the CHC trait
pub trait ChainHeadCoordinatorExt:
    'static + Send + Sync + ChainHeadCoordinator<Item = SignedActionHashed>
{
    /// Get info necessary for signing
    fn signing_info(&self) -> (MetaLairClient, AgentPubKey);

    /// More convenient way to call `add_records_request`
    fn add_records(self: Arc<Self>, records: Vec<Record>) -> MustBoxFuture<'static, ChcResult<()>> {
        let (keystore, agent) = self.signing_info();
        async move {
            let payload = AddRecordPayload::from_records(keystore, agent, records).await?;
            self.add_records_request(payload).await
        }
        .boxed()
        .into()
    }

    /// More convenient way to call the low-level CHC API method `get_record_data_request`.
    /// This method actually decodes and assembles those results into a list of `Record`s.
    fn get_record_data(
        self: Arc<Self>,
        since_hash: Option<ActionHash>,
    ) -> MustBoxFuture<'static, ChcResult<Vec<Record>>> {
        let (keystore, agent) = self.signing_info();
        async move {
            let mut bytes = [0; 32];
            getrandom::getrandom(&mut bytes).map_err(|e| ChcError::Other(e.to_string()))?;
            let nonce = Nonce256Bits::from(bytes);
            let payload = GetRecordsPayload { since_hash, nonce };
            let signature = agent.sign(&keystore, &payload).await?;
            self.get_record_data_request(GetRecordsRequest { payload, signature })
                .await?
                .into_iter()
                .map(|(a, me)| {
                    Ok(Record::new(
                        a,
                        me.map(|(e, _s)| holochain_serialized_bytes::decode(&e.0))
                            .transpose()?,
                    ))
                })
                .collect()
        }
        .boxed()
        .into()
    }

    /// Just a convenience for testing. Should not be used otherwise.
    #[cfg(feature = "test_utils")]
    fn head(self: Arc<Self>) -> MustBoxFuture<'static, ChcResult<Option<ActionHash>>> {
        async move {
            Ok(self
                .get_record_data(None)
                .await?
                .pop()
                .map(|r| r.action_address().clone()))
        }
        .boxed()
        .into()
    }
}

/// A CHC implementation
pub type ChcImpl = Arc<dyn 'static + Send + Sync + ChainHeadCoordinatorExt>;

/// A Record to be added to the CHC.
///
/// The SignedActionHashed is constructed as usual.
/// The Entry data is encrypted (TODO: by which key?), and the encrypted data
/// is signed by the agent. This ensures that only the correct agent is adding
/// records to its CHC. This EncryptedEntry signature is not used anywhere
/// outside the context of the CHC.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AddRecordPayload {
    /// The msgpack-encoded SignedActionHashed for the Record. This is encoded as such because the CHC
    /// needs to verify the signature, and these are the exact bytes which are signed, so
    /// this removes the need to deserialize and then re-serialize.
    ///
    /// This must be deserialized as `SignedActionHashed`.
    #[serde(with = "serde_bytes")]
    pub signed_action_msgpack: Vec<u8>,

    /// The signature of the SignedActionHashed
    /// (NOTE: usually signatures are of just the Action, but in this case we want to
    /// include the entire struct in the signature so we don't have to recalculate that on the CHC)
    pub signed_action_signature: Signature,

    /// The entry, encrypted (TODO: by which key?), with the signature of
    /// of the encrypted bytes
    pub encrypted_entry: Option<(Arc<EncryptedEntry>, Signature)>,
}

impl AddRecordPayload {
    /// Create a payload from a list of records.
    /// This performs the necessary signing and encryption the CHC requires.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(keystore, records)))]
    pub async fn from_records(
        keystore: MetaLairClient,
        agent_pubkey: AgentPubKey,
        records: Vec<Record>,
    ) -> ChcResult<Vec<Self>> {
        futures::future::join_all(records.into_iter().map(
            |Record {
                 signed_action,
                 entry,
             }| {
                let keystore = keystore.clone();
                let agent_pubkey = agent_pubkey.clone();

                async move {
                    let encrypted_entry_bytes = entry
                        .into_option()
                        .map(|entry| {
                            let entry = holochain_serialized_bytes::encode(&entry)?;
                            tracing::warn!(
                                "CHC is using unencrypted entry data. TODO: add encryption"
                            );

                            ChcResult::Ok(entry)
                        })
                        .transpose()?;
                    let encrypted_entry = if let Some(bytes) = encrypted_entry_bytes {
                        let signature = keystore
                            .sign(agent_pubkey.clone(), bytes.clone().into())
                            .await?;
                        Some((Arc::new(bytes.into()), signature))
                    } else {
                        None
                    };
                    let signed_action_msgpack = holochain_serialized_bytes::encode(&signed_action)?;
                    let author = signed_action.action().author();

                    let signed_action_signature = author
                        .sign_raw(&keystore, signed_action_msgpack.clone().into())
                        .await?;

                    assert!(author
                        .verify_signature_raw(
                            &signed_action_signature,
                            signed_action_msgpack.clone().into()
                        )
                        .await
                        .unwrap());
                    ChcResult::Ok(AddRecordPayload {
                        signed_action_msgpack,
                        signed_action_signature,
                        encrypted_entry,
                    })
                }
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    }
}

/// The request type for `add_records`
pub type AddRecordsRequest = Vec<AddRecordPayload>;

/// The request to retrieve records from the CHC.
///
/// If a `since_hash` is specified, all records with sequence numbers at and
/// above the one at the given hash will be returned. If no `since_hash` is
/// given, then all records will be returned.
///
/// Since this payload is signed, including a unique nonce helps prevent replay
/// attacks.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetRecordsPayload {
    /// Only records beyond and including this hash are returned
    pub since_hash: Option<ActionHash>,
    /// Randomly selected nonce to prevent replay attacks
    pub nonce: Nonce256Bits,
}

/// The full request for get_record_data
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetRecordsRequest {
    /// The payload
    pub payload: GetRecordsPayload,
    /// The signature of the payload
    pub signature: Signature,
}

/// Encrypted bytes of an Entry
#[derive(Debug, serde::Serialize, serde::Deserialize, derive_more::From)]
pub struct EncryptedEntry(#[serde(with = "serde_bytes")] pub Vec<u8>);

/// Assemble records from a list of Actions and a map of Entries
pub fn records_from_actions_and_entries(
    actions: Vec<SignedActionHashed>,
    mut entries: HashMap<EntryHash, Entry>,
) -> ChcResult<Vec<Record>> {
    let mut records = vec![];
    for action in actions {
        let entry = if let Some(hash) = action.hashed.entry_hash() {
            Some(
                entries
                    .remove(hash)
                    .ok_or_else(|| ChcError::MissingEntryForAction(action.as_hash().clone()))?,
            )
        } else {
            None
        };
        let record = Record::new(action, entry);
        records.push(record);
    }
    Ok(records)
}

#[allow(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum ChcError {
    #[error(transparent)]
    SerializationError(#[from] SerializedBytesError),

    #[error(transparent)]
    JsonSerializationError(#[from] serde_json::Error),

    #[error(transparent)]
    LairError(#[from] one_err::OneErr),

    /// The out of sync error only happens when you attempt to add actions
    /// that would cause a fork with respect to the CHC. This can be remedied
    /// by syncing.
    #[error("Local chain is out of sync with the CHC. The CHC head has advanced beyond the first action provided in the `add_records` request. Try calling `get_record_data` from hash {1} (sequence #{0}).")]
    InvalidChain(u32, ActionHash),

    /// All other errors are due to an invalid request, which is a mistake
    /// that can't be remedied other than by fixing the programming mistake
    /// (which would be on the Holochain side)
    /// Examples include:
    /// - `Vec<AddRecordPayload>` must be sorted by `seq_number`
    /// - There is a gap between the first action and the current CHC head
    /// - The `Vec<AddRecordPayload>` does not constitute a valid chain (prev_action must be correct)
    #[error("Invalid `add_records` payload. Seq number: {0}")]
    NoRecordsAdded(u32),

    /// An Action which has an entry was returned without the Entry
    #[error("Missing Entry for ActionHash: {0}")]
    MissingEntryForAction(ActionHash),

    #[error("The CHC service is unreachable: {0}")]
    ServiceUnreachable(String),

    /// Unexpected error
    #[error("Unexpected error: {0}")]
    Other(String),
}

#[allow(missing_docs)]
pub type ChcResult<T> = Result<T, ChcError>;
