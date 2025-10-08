//! Defines a Record, the basic unit of Holochain data.

use crate::action::WireDelete;
use crate::action::WireUpdateRelationship;
use crate::prelude::*;
use holochain_keystore::KeystoreError;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use holochain_zome_types::entry::EntryHashed;

/// A condensed version of get record request.
/// This saves bandwidth by removing duplicated and implied data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
pub struct WireRecordOps {
    /// The action this request was for.
    pub action: Option<Judged<SignedAction>>,
    /// Any deletes on the action.
    pub deletes: Vec<Judged<WireDelete>>,
    /// Any updates on the action.
    pub updates: Vec<Judged<WireUpdateRelationship>>,
    /// The entry if there is one.
    pub entry: Option<Entry>,
}

impl WireRecordOps {
    /// Create an empty set of wire record ops.
    pub fn new() -> Self {
        Self::default()
    }
    /// Render these ops to their full types.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            action,
            deletes,
            updates,
            entry,
        } = self;
        let mut ops = Vec::with_capacity(1 + deletes.len() + updates.len());
        if let Some(action) = action {
            let status = action.validation_status();
            let (action, signature) = action.data.into();
            // TODO: If they only need the metadata because they already have
            // the content we could just send the entry hash instead of the
            // SignedAction.
            let entry_hash = action.entry_hash().cloned();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::StoreRecord,
            )?);
            if let Some(entry_hash) = entry_hash {
                for op in deletes {
                    let status = op.validation_status();
                    let op = op.data;
                    let signature = op.signature;
                    let action = Action::Delete(op.delete);

                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        ChainOpType::RegisterDeletedBy,
                    )?);
                }
                for op in updates {
                    let status = op.validation_status();
                    let (action, signature) = op.data.into_signed_action(entry_hash.clone()).into();

                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        ChainOpType::RegisterUpdatedRecord,
                    )?);
                }
            }
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

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait SignedActionHashedExt {
    /// Create a hash from data
    fn from_content_sync(signed_action: SignedAction) -> SignedActionHashed;
    /// Sign some content
    #[allow(clippy::new_ret_no_self)]
    async fn sign(
        keystore: &MetaLairClient,
        action: ActionHashed,
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
    async fn sign(keystore: &MetaLairClient, action_hashed: ActionHashed) -> LairResult<Self> {
        let signature = action_hashed
            .signer()
            .sign(keystore, action_hashed.as_content())
            .await?;
        Ok(Self::with_presigned(action_hashed, signature))
    }

    /// Verify that the signature matches the signed action
    async fn verify_signature(&self) -> Result<(), KeystoreError> {
        if !self
            .action()
            .signer()
            .verify_signature(self.signature(), self.action())
            .await?
        {
            return Err(KeystoreError::InvalidSignature(
                self.signature().clone(),
                format!("action {:?}", self.action_address()),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SignedAction;
    use super::SignedActionHashed;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use holo_hash::HasHash;
    use holo_hash::HoloHashed;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_signed_action_roundtrip() {
        let signature = SignatureFixturator::new(Unpredictable).next().unwrap();
        let action = ActionFixturator::new(Unpredictable).next().unwrap();
        let signed_action = SignedAction::new(action, signature);
        let hashed: HoloHashed<SignedAction> = HoloHashed::from_content_sync(signed_action);
        let HoloHashed { content, hash } = hashed.clone();
        let (action, signature) = content.into();
        let shh = SignedActionHashed {
            hashed: ActionHashed::with_pre_hashed(action, hash),
            signature,
        };

        assert_eq!(shh.action_address(), hashed.as_hash());

        let round = HoloHashed {
            content: SignedAction::new(shh.action().clone(), shh.signature().clone()),
            hash: shh.action_address().clone(),
        };

        assert_eq!(hashed, round);
    }
}
