use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use holo_hash::AgentPubKey;
use holochain_conductor_api::ZomeCallParamsSigned;
use holochain_zome_types::{
    capability::CapSecret,
    cell::CellId,
    dependencies::holochain_integrity_types::Signature,
    zome_io::{ExternIO, ZomeCallParams},
};

pub(crate) mod client_signing;

#[cfg(feature = "lair_signing")]
pub(crate) mod lair_signing;

pub type DynAgentSigner = Arc<dyn AgentSigner + Send + Sync>;

#[async_trait]
pub trait AgentSigner {
    /// Sign the given data with the public key found in the agent id of the provenance.
    async fn sign(
        &self,
        cell_id: &CellId,
        provenance: AgentPubKey,
        data_to_sign: Arc<[u8]>,
    ) -> Result<Signature>;

    fn get_provenance(&self, cell_id: &CellId) -> Option<AgentPubKey>;

    /// Get the capability secret for the given `cell_id` if it exists.
    fn get_cap_secret(&self, cell_id: &CellId) -> Option<CapSecret>;
}

/// Signs an unsigned zome call using the provided signing implementation
pub(crate) async fn sign_zome_call(
    params: ZomeCallParams,
    signer: DynAgentSigner,
) -> Result<ZomeCallParamsSigned> {
    let pub_key = params.provenance.clone();
    let (bytes, bytes_hash) = params.serialize_and_hash()?;
    let signature = signer
        .sign(&params.cell_id, pub_key, bytes_hash.into())
        .await?;

    Ok(ZomeCallParamsSigned {
        bytes: ExternIO(bytes),
        signature,
    })
}
