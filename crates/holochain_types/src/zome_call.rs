//! Data needed to make zome calls.
use crate::prelude::*;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use std::sync::Arc;

/// Zome calls need to be signed regardless of how they are called.
/// This defines exactly what needs to be signed.
#[derive(Serialize, Deserialize, Debug)]
pub struct ZomeCallUnsigned {
    /// Provenance to sign.
    pub provenance: AgentPubKey,
    /// Cell ID to sign.
    pub cell_id: CellId,
    /// Zome name to sign.
    pub zome_name: ZomeName,
    /// Function name to sign.
    pub fn_name: FunctionName,
    /// Cap secret to sign.
    pub cap_secret: Option<CapSecret>,
    /// Payload to sign.
    pub payload: ExternIO,
    /// Nonce to sign.
    pub nonce: IntNonce,
    /// Time after which this zome call MUST NOT be accepted.
    pub expires_at: Timestamp,
}

impl ZomeCallUnsigned {
    /// Prepare the canonical bytes for an unsigned zome call so that it is
    /// always signed and verified in the same way.
    pub fn data_to_sign(&self) -> Result<Arc<[u8]>, SerializedBytesError> {
        Ok(holo_hash::encode::blake2b_256(&holochain_serialized_bytes::encode(&self)?).into())
    }

    /// Sign the unsigned zome call in a canonical way to produce a signature.
    pub async fn sign(&self, keystore: &MetaLairClient) -> LairResult<Signature> {
        self.provenance
            .sign_raw(
                keystore,
                self.data_to_sign()
                    .map_err(|e| one_err::OneErr::new(e.to_string()))?,
            )
            .await
    }
}
