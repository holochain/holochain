use crate::cursor::ChainCursorX;
use crate::{agent::SourceChain, ribosome::Ribosome, nucleus::{ZomeInvocation, ZomeInvocationResult}};
use sx_types::error::SkunkResult;
use sx_types::shims::*;

pub async fn invoke_zome(
    invocation: ZomeInvocation,
    source_chain: SourceChain<'_>,
    cursor: ChainCursorX,
) -> SkunkResult<ZomeInvocationResult> {
    let dna = source_chain.dna()?;
    let ribosome = Ribosome::new(dna);
    let (result, cursor) = ribosome.invoke_zome(cursor, invocation, source_chain.now())?;
    source_chain.try_commit(cursor)?;
    Ok(result)
}
