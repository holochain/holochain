use super::HostContext;
use super::WasmRibosome;
use crate::core::state::cascade::Cascade;
use holochain_types::test_utils::fake_agent_pubkey_1;
use holochain_zome_types::GetEntryInput;
use holochain_zome_types::GetEntryOutput;
use std::sync::Arc;

pub async fn get_entry(
    _ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    _input: GetEntryInput,
) -> GetEntryOutput {
    let call = |_cascade: &Cascade| {
        let _agent_pubkey = fake_agent_pubkey_1();
        // FIXME: This can't be borrowed in the future returned here
        // because the closure does not have a static liftime
        //cascade.dht_get(agent_pubkey.into()).boxed()
    };
    let _entry = unsafe { host_context.cascade.apply_ref(call) };
    unimplemented!();
}
