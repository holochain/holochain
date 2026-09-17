use crate::{AgentPubKeyExt, KeystoreError, LairResult, MetaLairClient};
use holo_hash::HashableContentExtSync;
use holochain_types::prelude::{Action, SignedAction, SignedActionHashed};

/// Extension for keystore operations on a [`SignedActionHashed`].
#[async_trait::async_trait]
pub trait SignedActionHashedExt {
    /// Create a hash from data
    fn from_content_sync(signed_action: SignedAction) -> SignedActionHashed;
    /// Sign some content
    async fn sign(
        keystore: &MetaLairClient,
        action: holo_hash::HoloHashed<Action>,
    ) -> LairResult<SignedActionHashed>;
    /// Validate the data
    async fn verify_signature(&self) -> Result<(), KeystoreError>;
}

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
        let signature = action_hashed
            .content
            .author()
            .sign(keystore, &action_hashed.content)
            .await?;
        Ok(Self::with_presigned(action_hashed, signature))
    }

    /// Verify that the signature matches the signed action
    async fn verify_signature(&self) -> Result<(), KeystoreError> {
        if !self
            .hashed
            .content
            .author()
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
mod test {
    use crate::SignedActionHashedExt;
    use holo_hash::{AgentPubKey, HoloHashed};
    use holochain_types::prelude::*;
    use holochain_zome_types::prelude::{SignedAction, SignedActionHashed};

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

    #[test]
    fn signed_action_round_trip() {
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
