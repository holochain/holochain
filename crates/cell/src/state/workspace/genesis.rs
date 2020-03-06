

use sx_state::Reader;
use crate::state::source_chain::SourceChainBuffer;

pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuffer<'env, Reader<'env>>,
    // meta: KvvBuffer<'env, String, String>,
}
