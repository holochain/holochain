use crate::capability::CapSecret;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::AgentPubKey;
use crate::prelude::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CallRemote {
    to_agent: AgentPubKey,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap: Option<CapSecret>,
    payload: ExternIO,
}

impl CallRemote {
    pub fn new(
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> Self {
        Self {
            to_agent,
            zome_name,
            fn_name,
            cap,
            payload,
        }
    }

    pub fn as_to_agent(&self) -> &AgentPubKey {
        &self.to_agent
    }

    pub fn as_zome_name(&self) -> &ZomeName {
        &self.zome_name
    }

    pub fn as_fn_name(&self) -> &FunctionName {
        &self.fn_name
    }

    pub fn as_cap(&self) -> &Option<CapSecret> {
        &self.cap
    }

    pub fn as_payload(&self) -> &ExternIO {
        &self.payload
    }
}
