use crate::state::source_chain::SourceChainBuffer;
use sx_state::Reader;

pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuffer<'env, Reader<'env>>,
    // meta: KvvBuffer<'env, String, String>,
}

#[cfg(tests)]
pub mod tests {


    #[test]
    fn can_perform_genesis() {

    }
}
