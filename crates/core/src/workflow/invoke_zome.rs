use crate::{
    agent::{ChainTop, SourceChain, SourceChainSnapshot},
    cell::{autonomic::AutonomicCue, error::CellResult},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    ribosome::{Ribosome, RibosomeT},
    txn::source_chain, conductor_api::ConductorCellApiT,
};
use sx_types::shims::*;

pub async fn invoke_zome<Ribo: RibosomeT>(
    invocation: ZomeInvocation,
    source_chain: SourceChain<'_>,
    ribosome: Ribo,
) -> CellResult<(ZomeInvocationResult, SourceChainSnapshot)> {
    let bundle = source_chain.bundle()?;
    let (result, bundle) = ribosome.call_zome_function(bundle, invocation)?;
    let snapshot = source_chain.try_commit(bundle)?;
    Ok((result, snapshot))
}


#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::agent::SourceChainCommitBundle;
    use sx_types::{entry::Entry, error::SkunkResult};

    struct FakeRibosome;

    impl RibosomeT for FakeRibosome {
        fn run_validation(self, cursor: &source_chain::Cursor, entry: Entry) -> ValidationResult {
            unimplemented!()
        }

        /// Runs the specified zome fn. Returns the cursor used by HDK,
        /// so that it can be passed on to source chain manager for transactional writes
        fn call_zome_function(
            self,
            bundle: SourceChainCommitBundle,
            invocation: ZomeInvocation,
        ) -> SkunkResult<(ZomeInvocationResult, SourceChainCommitBundle)> {
            unimplemented!()
        }
    }
}
