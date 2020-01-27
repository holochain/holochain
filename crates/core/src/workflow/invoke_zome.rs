use crate::error::SkunkResult;
use sx_types::shims::*;
use crate::cursor::CasCursorX;
use crate::types::ZomeInvocationResult;
use crate::{agent::SourceChain, types::ZomeInvocation, ribosome::Ribosome};

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain,
    cursor: CasCursorX,
) -> SkunkResult<ZomeInvocationResult> {
    let dna = source_chain.get_dna()?;
    let ribosome = Ribosome::new(dna);
    let (result, cursor) = ribosome.call_zome_function(cursor, invocation, source_chain.clone())?;
    source_chain.try_commit(cursor)?;
    Ok(result)
}
