use crate::{state::workspace::GenesisWorkspace, cell::error::CellResult};
use sx_types::{dna::Dna, prelude::*, agent::AgentId};
use super::WorkflowResult;

type WS<'env> = GenesisWorkspace<'env>;

/// Initialize the source chain with the initial entries:
/// - Dna
/// - AgentId
/// - CapTokenGrant
pub async fn genesis<'env>(workspace: WS<'env>, dna: Dna, agent_id: AgentId) -> WorkflowResult<WS<'env>> {
    unimplemented!()
}
