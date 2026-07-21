//! Types for validation receipts and signed validation receipts to be sent between peers.

use crate::prelude::{Signature, Timestamp};
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;
use std::vec::IntoIter;

/// Validation receipt content - to be signed.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
pub struct ValidationReceipt {
    /// the op this validation receipt is for.
    pub dht_op_hash: DhtOpHash,

    /// the result of this validation.
    pub validation_status: ValidationStatus,

    /// the remote validator which is signing this receipt.
    pub validators: Vec<AgentPubKey>,

    /// Time when the op was integrated
    pub when_integrated: Timestamp,
}

/// A full, signed validation receipt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
pub struct SignedValidationReceipt {
    /// the content of the validation receipt.
    pub receipt: ValidationReceipt,

    // TODO This is just the signature and not the original message, should this be a full signature and get validated
    //      when it is received? https://github.com/holochain/holochain/pull/2848#discussion_r1346160783
    /// the signature of the remote validator.
    pub validators_signatures: Vec<Signature>,
}

/// A bundle of validation receipts to be sent together.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct ValidationReceiptBundle(Vec<SignedValidationReceipt>);

impl From<Vec<SignedValidationReceipt>> for ValidationReceiptBundle {
    fn from(value: Vec<SignedValidationReceipt>) -> Self {
        ValidationReceiptBundle(value)
    }
}

impl IntoIterator for ValidationReceiptBundle {
    type Item = SignedValidationReceipt;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
