use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::zome_info::ZomeInfo;
use holochain_zome_types::ZomeInfoInput;
use holochain_zome_types::ZomeInfoOutput;
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
        zome_id: ribosome.zome_name_to_id(&call_context.zome_name)?,
        properties: ribosome.dna_file().dna().properties.clone(),
        // @TODO
        // public_token: "".into(),
    }))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::ZomeInfoOutput;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_zome_info_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let zome_info: ZomeInfoOutput =
            crate::call_test_ribosome!(host_access, TestWasm::ZomeInfo, "zome_info", ());
        assert_eq!(zome_info.inner_ref().dna_name, "test",);
    }
}
