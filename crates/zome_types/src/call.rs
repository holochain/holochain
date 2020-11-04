use crate::capability::CapSecret;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_serialized_bytes::prelude::SerializedBytes;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Call {
    pub to_agent: AgentPubKey,
    pub to_dna: Option<DnaHash>,
    pub zome_name: ZomeName,
    pub fn_name: FunctionName,
    pub cap: Option<CapSecret>,
    pub request: SerializedBytes,
    pub provenance: AgentPubKey,
}

impl Call {
    pub fn new(
        to_agent: AgentPubKey,
        to_dna: Option<DnaHash>,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        request: SerializedBytes,
        provenance: AgentPubKey,
    ) -> Self {
        Self {
            to_agent,
            to_dna,
            zome_name,
            fn_name,
            cap,
            request,
            provenance,
        }
    }
}
