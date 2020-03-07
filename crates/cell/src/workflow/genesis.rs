use super::WorkflowResult;
use crate::state::workspace::GenesisWorkspace;
use sx_types::{agent::AgentId, dna::Dna};

type WS<'env> = GenesisWorkspace<'env>;

/// Initialize the source chain with the initial entries:
/// - Dna
/// - AgentId
/// - CapTokenGrant
pub async fn genesis<'env>(
    _workspace: WS<'env>,
    _dna: Dna,
    _agent_id: AgentId,
) -> WorkflowResult<WS<'env>> {
    unimplemented!()
}
