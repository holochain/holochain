use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, ribosome::Ribosome, types::ZomeInvocation};
use sx_types::error::SkunkResult;
#[allow(unused_imports)]
use sx_types::shims::*;
use crate::txn::source_chain;

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain,
    cursor_rw: source_chain::CursorRw,
) -> SkunkResult<ZomeInvocationResult> {
    let dna = source_chain.get_dna()?;
    let ribosome = Ribosome::new(dna);
    let (result, cursor_rw) = ribosome.call_zome_function(cursor_rw, invocation, source_chain.clone())?;
    source_chain.try_commit(cursor_rw)?;
    Ok(result)
}
