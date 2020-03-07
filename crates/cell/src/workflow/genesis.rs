use super::WorkflowResult;
use crate::state::workspace::GenesisWorkspace;
use sx_types::{agent::AgentId, dna::Dna};

type WS<'env> = GenesisWorkspace<'env>;

/// Initialize the source chain with the initial entries:
/// - Dna
/// - AgentId
/// - CapTokenGrant
pub async fn genesis<'env>(
    workspace: WS<'env>,
    dna: Dna,
    agent_id: AgentId,
) -> WorkflowResult<WS<'env>> {
    unimplemented!()
}
