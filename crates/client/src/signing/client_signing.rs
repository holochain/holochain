use super::{AgentSigner, DynAgentSigner};
use async_trait::async_trait;
use ed25519_dalek::Signer;
use holo_hash::AgentPubKey;
use holochain_zome_types::{
    capability::CapSecret, cell::CellId, dependencies::holochain_integrity_types::Signature,
};
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};

pub struct SigningCredentials {
    pub signing_agent_key: AgentPubKey,
    pub keypair: ed25519_dalek::SigningKey,
    pub cap_secret: CapSecret,
}

/// Custom debug implementation which won't attempt to print the `cap_secret` or `keypair`
impl std::fmt::Debug for SigningCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningCredentials")
            .field("signing_agent_key", &self.signing_agent_key)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClientAgentSigner {
    credentials: Arc<RwLock<HashMap<CellId, SigningCredentials>>>,
}

impl ClientAgentSigner {
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_credentials(&self, cell_id: CellId, credentials: SigningCredentials) {
        self.credentials.write().insert(cell_id, credentials);
    }
}

#[async_trait]
impl AgentSigner for ClientAgentSigner {
    async fn sign(
        &self,
        cell_id: &CellId,
        _provenance: AgentPubKey,
        data_to_sign: Arc<[u8]>,
    ) -> Result<Signature, anyhow::Error> {
        let credentials_lock = self.credentials.read();
        let credentials = credentials_lock
            .get(cell_id)
            .ok_or_else(|| anyhow::anyhow!("No credentials found for cell: {cell_id:?}"))?;
        let signature = credentials.keypair.try_sign(&data_to_sign)?;
        Ok(Signature(signature.to_bytes()))
    }

    fn get_provenance(&self, cell_id: &CellId) -> Option<AgentPubKey> {
        self.credentials
            .read()
            .get(cell_id)
            .map(|c| c.signing_agent_key.clone())
    }

    fn get_cap_secret(&self, cell_id: &CellId) -> Option<CapSecret> {
        self.credentials.read().get(cell_id).map(|c| c.cap_secret)
    }
}

/// Convert the ClientAgentSigner into an `Arc<Box<dyn AgentSigner + Send + Sync>>`
impl From<ClientAgentSigner> for DynAgentSigner {
    fn from(cas: ClientAgentSigner) -> Self {
        Arc::new(cas)
    }
}
