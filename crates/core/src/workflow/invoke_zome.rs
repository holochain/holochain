use crate::shims::CascadingCursor;
use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, cell::Cell, shims::call_zome_function, types::ZomeInvocation};
use futures::never::Never;
use skunkworx_core_types::error::SkunkResult;

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain,
    cursor: CascadingCursor,
) -> SkunkResult<ZomeInvocationResult> {
    // let mut cursor = source_chain.get_cursor();
    // let ribosome = get_ribosome()
    // ribosome.call_zome_function(invocation, source_chain);
    // source_chain.try_commit(cursor);

    Ok(ZomeInvocationResult)
}
