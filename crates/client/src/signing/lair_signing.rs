use crate::AgentSigner;
use anyhow::Result;
use async_trait::async_trait;
use holo_hash::AgentPubKey;
use holochain_zome_types::{
    capability::CapSecret, cell::CellId, dependencies::holochain_integrity_types::Signature,
};
use lair_keystore_api::LairClient;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub struct LairAgentSigner {
    lair_client: Arc<LairClient>,
    credentials: Arc<RwLock<HashMap<CellId, AgentPubKey>>>,
}

impl LairAgentSigner {
    pub fn new(lair_client: Arc<LairClient>) -> Self {
        Self {
            lair_client,
            credentials: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add credentials for a cell to the signer.
    /// The provenance should be the `agent_pub_key` that the cell is running as.
    pub fn add_credentials(&mut self, cell_id: CellId, provenance: AgentPubKey) {
        self.credentials.write().insert(cell_id, provenance);
    }
}

#[async_trait]
impl AgentSigner for LairAgentSigner {
    async fn sign(
        &self,
        _cell_id: &CellId,
        provenance: AgentPubKey,
        data_to_sign: Arc<[u8]>,
    ) -> Result<Signature> {
        let public_key: [u8; 32] = provenance.get_raw_32().try_into()?;

        let signature = self
            .lair_client
            .sign_by_pub_key(public_key.into(), None, data_to_sign)
            .await?;

        Ok(Signature(*signature.0))
    }

    fn get_provenance(&self, cell_id: &CellId) -> Option<AgentPubKey> {
        self.credentials.read().get(cell_id).cloned()
    }

    /// Not used with Lair signing. If you have access to Lair then you don't need to prove you
    // are supposed to have access to a specific key pair.
    fn get_cap_secret(&self, _cell_id: &CellId) -> Option<CapSecret> {
        None
    }
}
