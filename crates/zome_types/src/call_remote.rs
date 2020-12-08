use crate::capability::CapSecret;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::AgentPubKey;
use holochain_serialized_bytes::prelude::SerializedBytes;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CallRemote {
    to_agent: AgentPubKey,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap: Option<CapSecret>,
    request: SerializedBytes,
}

impl CallRemote {
    pub fn new(
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
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

    pub fn fn_name(&self) -> FunctionName {
        self.fn_name.clone()
    }

    pub fn cap(&self) -> Option<CapSecret> {
        self.cap
    }

    pub fn request(&self) -> SerializedBytes {
        self.request.clone()
    }
}
