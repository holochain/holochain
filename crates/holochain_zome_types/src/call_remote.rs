use crate::capability::CapSecret;
use crate::prelude::*;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::AgentPubKey;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CallRemote {
    target_agents: Vec<AgentPubKey>,
    zome_name: ZomeName,
    fn_name: FunctionName,
    cap: Option<CapSecret>,
    payload: ExternIO,
}

impl CallRemote {
    pub fn new(
        target_agents: Vec<AgentPubKey>,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> Self {
        Self {
            target_agents,
            zome_name,
            fn_name,
            cap,
            payload,
        }
    }

    pub fn target_agents(&self) -> &Vec<AgentPubKey> {
        &self.target_agents
    }

    pub fn zome_name(&self) -> &ZomeName {
        &self.zome_name
    }

    pub fn fn_name(&self) -> &FunctionName {
        &self.fn_name
    }

    pub fn cap(&self) -> &Option<CapSecret> {
        &self.cap
    }

    pub fn payload(&self) -> &ExternIO {
        &self.payload
    }
}
