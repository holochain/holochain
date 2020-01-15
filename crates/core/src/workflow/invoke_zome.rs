use crate::shims::*;
use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, cell::Cell, shims::call_zome_function, types::ZomeInvocation};
use futures::never::Never;
use skunkworx_core_types::error::SkunkResult;

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain,
    cursor: CascadingCursor,
) -> SkunkResult<ZomeInvocationResult> {
    let dna = source_chain.get_dna()?;
    let ribosome = Ribosome::new(dna, cursor);
    let result = ribosome.call_zome_function(invocation, source_chain)?;
    source_chain.try_commit(cursor);
    Ok(result)
}
