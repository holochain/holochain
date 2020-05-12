use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::DnaDef;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct MigrateAgentInvocation {
    // @todo MigrateAgentWorkspace?
    workspace: UnsafeInvokeZomeWorkspace,
    dna_def: DnaDef,
    migrate_agent: MigrateAgent,
}

impl Invocation for MigrateAgentInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::No
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        self.dna_def.zomes.keys().cloned().collect()
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "migrate_agent".into(),
            match self.migrate_agent {
                MigrateAgent::Open => "open",
                MigrateAgent::Close => "close",
            }
            .into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.migrate_agent).try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
    }
}

impl TryFrom<MigrateAgentInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(migrate_agent_invocation: MigrateAgentInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new(
            (&migrate_agent_invocation.migrate_agent).try_into()?,
        ))
    }
}

/// the aggregate result of all zome callbacks for migrating an agent between dnas
pub enum MigrateAgentResult {
    /// all implemented migrate agent callbacks in all zomes passed
    Pass,
    /// some migrate agent callback failed
    /// ZomeName is the first zome that failed
    /// String is some human readable string explaining the failure
    Fail(ZomeName, String),
}

impl From<Vec<MigrateAgentCallbackResult>> for MigrateAgentResult {
    fn from(callback_results: Vec<MigrateAgentCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Pass, |acc, x| {
            match x {
                // fail always overrides the acc
                MigrateAgentCallbackResult::Fail(zome_name, fail_string) => {
                    Self::Fail(zome_name, fail_string)
                }
                // pass allows the acc to continue
                MigrateAgentCallbackResult::Pass => acc,
            }
        })
    }
}
