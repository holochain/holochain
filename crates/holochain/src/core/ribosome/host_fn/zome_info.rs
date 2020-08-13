use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::zome_info::ZomeInfo;
use holochain_zome_types::ZomeInfoInput;
use holochain_zome_types::ZomeInfoOutput;
use std::convert::TryFrom;
use std::sync::Arc;

pub fn zome_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: ZomeInfoInput,
) -> RibosomeResult<ZomeInfoOutput> {
    Ok(ZomeInfoOutput::new(ZomeInfo {
        dna_name: ribosome.dna_file().dna().name.clone(),
        zome_name: call_context.zome_name.clone(),
        dna_hash: ribosome.dna_file().dna_hash().clone(), // @TODO
        properties: SerializedBytes::try_from(()).unwrap(), // @TODO
                                                          // @todo
                                                          // public_token: "".into(),                            // @TODO
    }))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::core::{
        state::workspace::Workspace, workflow::unsafe_call_zome_workspace::CallZomeWorkspaceFactory,
    };
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::{ZomeInfoInput, ZomeInfoOutput};

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_zome_info_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let mut host_access = fixt!(ZomeCallHostAccess);
        let factory: CallZomeWorkspaceFactory = env.clone().into();
        host_access.workspace = factory.clone();

        let zome_info: ZomeInfoOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::Imports,
            "zome_info",
            ZomeInfoInput::new(())
        );
        assert_eq!(zome_info.inner_ref().dna_name, "test",);
    }
}
