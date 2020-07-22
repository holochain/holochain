use crate::capability::CapSecret;
use crate::zome::ZomeName;
use holo_hash_core::AgentPubKey;
use holo_hash_core::DnaHash;
use holochain_serialized_bytes::prelude::SerializedBytes;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CallRemote {
    dna_hash: DnaHash,
    to_agent: AgentPubKey,
    zome_name: ZomeName,
    fn_name: String,
    cap: CapSecret,
    request: SerializedBytes,
}

impl CallRemote {
    pub fn dna_hash(&self) -> DnaHash {
        self.dna_hash.clone()
    }

    pub fn to_agent(&self) -> AgentPubKey {
        self.to_agent.clone()
    }

    pub fn zome_name(&self) -> ZomeName {
        self.zome_name.clone()
    }

    pub fn fn_name(&self) -> String {
        self.fn_name.clone()
    }

    pub fn cap(&self) -> CapSecret {
        self.cap.clone()
    }

    pub fn request(&self) -> SerializedBytes {
        self.request.clone()
    }
}
