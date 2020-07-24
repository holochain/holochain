use crate::capability::CapSecret;
use crate::zome::ZomeName;
use holo_hash_core::AgentPubKey;
use holochain_serialized_bytes::prelude::SerializedBytes;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CallRemote {
    to_agent: AgentPubKey,
    zome_name: ZomeName,
    fn_name: String,
    cap: CapSecret,
    request: SerializedBytes,
}

impl CallRemote {
    pub fn new(
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: String,
        cap: CapSecret,
        request: SerializedBytes,
    ) -> Self {
        Self {
            to_agent,
            zome_name,
            fn_name,
            cap,
            request,
        }
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
