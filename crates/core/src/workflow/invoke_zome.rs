use crate::{
    agent::SourceChain,
    cell::{autonomic::AutonomicCue, error::CellResult},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    ribosome::Ribosome,
    txn::source_chain, conductor_api::ConductorCellApiT,
};
use sx_types::shims::*;

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain<'_>,
) -> CellResult<ZomeInvocationResult> {
    let dna = source_chain.dna()?;
    let ribosome = Ribosome::new(dna);
    let bundle = source_chain.bundle()?;
    let (result, bundle) = ribosome.call_zome_function(bundle, invocation)?;
    source_chain.try_commit(bundle)?;
    Ok(result)
}
