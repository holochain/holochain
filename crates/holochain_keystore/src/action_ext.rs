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
    use crate::{test_keystore, AgentPubKeyExt, KeystoreError, SignedActionHashedExt};
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

    /// A `CloseChain` naming an agent migration target is signed by the chain
    /// author, like every other action. A signature by the target key must not
    /// verify; otherwise anyone could close another agent's chain in that
    /// agent's name (#5981).
    // `test_keystore()` spawns lair onto a blocking task, which requires a multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn close_chain_with_agent_target_is_signed_by_author() {
        let keystore = test_keystore();
        let author = AgentPubKey::new_random(&keystore).await.unwrap();
        let new_agent = AgentPubKey::new_random(&keystore).await.unwrap();

        let mut action = sample_action();
        action.header.author = author.clone();
        action.data = ActionData::CloseChain(CloseChainData {
            new_target: Some(MigrationTarget::Agent(new_agent.clone())),
        });
        let hashed: HoloHashed<Action> = HoloHashed::from_content_sync(action.clone());

        // Signing goes through the author key, and verifying checks it.
        let signed = SignedActionHashed::sign(&keystore, hashed.clone())
            .await
            .unwrap();
        assert!(author
            .verify_signature(signed.signature(), &action)
            .await
            .unwrap());
        assert!(!new_agent
            .verify_signature(signed.signature(), &action)
            .await
            .unwrap());
        signed.verify_signature().await.unwrap();

        // A signature by the migration target key is a forgery.
        let forged_sig = new_agent.sign(&keystore, &action).await.unwrap();
        let forged = SignedActionHashed::with_presigned(hashed, forged_sig);
        assert!(matches!(
            forged.verify_signature().await,
            Err(KeystoreError::InvalidSignature(..))
        ));
    }
}
