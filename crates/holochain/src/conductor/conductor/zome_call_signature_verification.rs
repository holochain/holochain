use crate::conductor::api::error::ConductorApiResult;
use holo_hash::{sha2_512, AgentPubKey};
use holochain_keystore::AgentPubKeyExt;
use holochain_types::prelude::Signature;

pub(crate) async fn is_valid_signature(
    provenance: &AgentPubKey,
    bytes: &[u8],
    signature: &Signature,
) -> ConductorApiResult<bool> {
    // Signature is verified against the hash of the signed zome call parameter bytes.
    let bytes_hash = sha2_512(bytes);
    Ok(provenance
        .verify_signature_raw(signature, bytes_hash.into())
        .await?)
}

#[cfg(test)]
mod tests {
    use super::is_valid_signature;
    use holo_hash::{sha2_512, AgentPubKey};
    use holochain_keystore::{test_keystore, AgentPubKeyExt};
    use holochain_types::prelude::Signature;

    #[tokio::test(flavor = "multi_thread")]
    async fn valid_signature() {
        let keystore = test_keystore();
        let agent_key = keystore.new_sign_keypair_random().await.unwrap();
        let bytes = vec![0u8];
        let bytes_hash = sha2_512(&bytes);
        let signature = agent_key
            .sign_raw(&keystore, bytes_hash.into())
            .await
            .unwrap();
        let is_valid = is_valid_signature(&agent_key, &bytes, &signature)
            .await
            .unwrap();
        assert!(is_valid);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_signature() {
        let keystore = test_keystore();
        let agent_key = keystore.new_sign_keypair_random().await.unwrap();
        let bytes = vec![0u8];
        let signature = Signature::from([0u8; 64]);
        let is_valid = is_valid_signature(&agent_key, &bytes, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn invalid_provenance() {
        let agent_key = AgentPubKey::from_raw_32(vec![0u8; 32]);
        let bytes = vec![0u8];
        let signature = Signature::from([0u8; 64]);
        let is_valid = is_valid_signature(&agent_key, &bytes, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn valid_signature_but_different_provenance() {
        let keystore = test_keystore();
        let signer_key = keystore.new_sign_keypair_random().await.unwrap();
        let bytes = vec![0u8];
        let bytes_hash = sha2_512(&bytes);
        let signature = signer_key.sign_raw(&keystore, bytes.into()).await.unwrap();
        let provenance = keystore.new_sign_keypair_random().await.unwrap();
        let is_valid = is_valid_signature(&provenance, &bytes_hash, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }
}
