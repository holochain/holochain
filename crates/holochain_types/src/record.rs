//! Defines a Record, the basic unit of Holochain data.

use crate::dht_v2::CloseChainData;
use crate::prelude::*;
use holochain_keystore::KeystoreError;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use holochain_zome_types::entry::EntryHashed;
use holochain_zome_types::warrant::SignedWarrant;

/// The record-serving response to a get-record request.
///
/// Serves the requested record as actions plus its entry (when public), each
/// action carrying its record-level validation status. A `Rejected` action is
/// always accompanied by a warrant in `warrants` proving the rejection; the
/// receiver checks that invariant up front before doing any validation work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
pub struct WireRecordOps {
    /// The action this request was for, with its validation status.
    pub action: Option<Judged<SignedAction>>,
    /// Any deletes on the action, each with its validation status.
    pub deletes: Vec<Judged<SignedAction>>,
    /// Any updates on the action, each with its validation status.
    pub updates: Vec<Judged<SignedAction>>,
    /// The entry if there is one.
    pub entry: Option<Entry>,
    /// Warrants proving any `Rejected` records served above.
    pub warrants: Vec<SignedWarrant>,
}

impl WireRecordOps {
    /// Create an empty set of wire record ops.
    pub fn new() -> Self {
        Self::default()
    }
    /// Expand the served records into the request-relevant ops for caching.
    ///
    /// Each served action becomes the single op the get-record request
    /// represents (`StoreRecord` for the record itself, `RegisterDeletedBy`
    /// per delete, `RegisterUpdatedRecord` per update), tagged with the served
    /// validation status. Warrants are handled separately by the requester.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            action,
            deletes,
            updates,
            entry,
            warrants: _,
        } = self;
        let mut ops = Vec::with_capacity(1 + deletes.len() + updates.len());
        if let Some(action) = action {
            let status = action.validation_status();
            let (action, signature) = action.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::StoreRecord,
            )?);
        }
        for op in deletes {
            let status = op.validation_status();
            let (action, signature) = op.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::RegisterDeletedBy,
            )?);
        }
        for op in updates {
            let status = op.validation_status();
            let (action, signature) = op.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::RegisterUpdatedRecord,
            )?);
        }
        Ok(RenderedOps {
            entry: entry.map(EntryHashed::from_content_sync),
            ops,
            warrant: None,
        })
    }
}

/// Record with it's status
#[derive(Debug, Clone, derive_more::Constructor)]
pub struct RecordStatus {
    /// The record this status applies to.
    pub record: Record,
    /// Validation status of this record.
    pub status: ValidationStatus,
}

/// The public key that should sign (and later verify) this action.
///
/// This is the author for every variant except a `CloseChain` that names an
/// agent migration target: that variant is signed with the *new* key so the
/// forward reference it carries is provably endorsed by the destination
/// agent (mirrors the legacy `Action::signer` special case).
fn action_signer(action: &Action) -> &AgentPubKey {
    match &action.data {
        ActionData::CloseChain(CloseChainData {
            new_target: Some(MigrationTarget::Agent(agent)),
            ..
        }) => agent,
        _ => &action.header.author,
    }
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait SignedActionHashedExt {
    /// Create a hash from data
    fn from_content_sync(signed_action: SignedAction) -> SignedActionHashed;
    /// Sign some content
    #[allow(clippy::new_ret_no_self)]
    async fn sign(
        keystore: &MetaLairClient,
        action: holo_hash::HoloHashed<Action>,
    ) -> LairResult<SignedActionHashed>;
    /// Validate the data
    async fn verify_signature(&self) -> Result<(), KeystoreError>;
}

#[allow(missing_docs)]
#[async_trait::async_trait]
impl SignedActionHashedExt for SignedActionHashed {
    fn from_content_sync(signed_action: SignedAction) -> Self
    where
        Self: Sized,
    {
        let (action, signature) = signed_action.into();
        Self::with_presigned(action.into_hashed(), signature)
    }

    /// Construct by signing the Action (NOT including the hash)
    async fn sign(
        keystore: &MetaLairClient,
        action_hashed: holo_hash::HoloHashed<Action>,
    ) -> LairResult<Self> {
        let signature = action_signer(&action_hashed.content)
            .sign(keystore, &action_hashed.content)
            .await?;
        Ok(Self::with_presigned(action_hashed, signature))
    }

    /// Verify that the signature matches the signed action
    async fn verify_signature(&self) -> Result<(), KeystoreError> {
        if !action_signer(&self.hashed.content)
            .verify_signature(self.signature(), &self.hashed.content)
            .await?
        {
            return Err(KeystoreError::InvalidSignature(
                self.signature().clone(),
                format!("action {:?}", self.as_hash()),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SignedAction;
    use super::SignedActionHashed;
    use super::SignedActionHashedExt;
    use crate::dht_v2::{ActionHeader, DnaData};
    use crate::prelude::*;
    use holo_hash::{AgentPubKey, DnaHash, HasHash, HoloHashed};

    fn sample_action() -> Action {
        Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_timestamp::Timestamp::from_micros(42),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::Dna(DnaData {
                dna_hash: DnaHash::from_raw_36(vec![2u8; 36]),
            }),
        }
    }

    #[tokio::test]
    async fn test_signed_action_roundtrip() {
        let signature = Signature([9u8; 64]);
        let action = sample_action();
        let signed_action = SignedAction::new(action.clone(), signature.clone());

        let shh = SignedActionHashed::from_content_sync(signed_action);

        assert_eq!(
            shh.as_hash(),
            &HoloHashed::<Action>::from_content_sync(action.clone()).into_hash()
        );
        assert_eq!(shh.hashed.content, action);
        assert_eq!(shh.signature, signature);
    }
}
