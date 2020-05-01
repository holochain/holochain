use super::HostContext;
use super::WasmRibosome;
use holochain_types::test_utils::fake_agent_hash;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> GetEntryOutput {
    let _entry = host_context.source_chain.apply_ref(|source_chain| {
        let agent_hash = fake_agent_hash("unsafe agent");
        source_chain.get_entry(agent_hash.into())
    });
    unimplemented!();
}
