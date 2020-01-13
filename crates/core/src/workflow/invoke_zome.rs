use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, cell::Cell, shims::call_zome_function, types::ZomeInvocation};
use futures::never::Never;
use skunkworx_core_types::error::SkunkResult;

pub async fn invoke_zome(
    _invocation: ZomeInvocation,
    _source_chain: SourceChain,
) -> SkunkResult<ZomeInvocationResult> {
    // let mut cursor = source_chain.get_cursor();
    // let ribosome = get_ribosome()
    // ribosome.call_zome_function(invocation, source_chain);
    // source_chain.try_commit(cursor);
    Ok(ZomeInvocationResult)
}
