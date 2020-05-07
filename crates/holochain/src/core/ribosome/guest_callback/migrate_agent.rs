use crate::core::ribosome::guest_callback::CallbackInvocation;
use holochain_types::dna::Dna;

pub struct MigrateAgentInvocation<'a> {
    dna: &'a Dna,
}

impl From<MigrateAgentInvocation<'_>> for CallbackInvocation<'_> {
    fn from(migrate_agent_invocation: MigrateAgentInvocation<'_>) -> Self {
        Self::MigrateAgent(migrate_agent_invocation)
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
