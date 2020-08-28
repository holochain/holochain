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

    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;

    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::ZomeInfoOutput;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_zome_info_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into(), &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        let zome_info: ZomeInfoOutput =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "zome_info", ());
        assert_eq!(zome_info.inner_ref().dna_name, "test",);
    }
}
