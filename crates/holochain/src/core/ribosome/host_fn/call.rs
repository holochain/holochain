use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::{error::RibosomeResult, ZomeCallInvocation};
use holochain_types::cell::CellId;
use holochain_zome_types::{CallInput, ZomeCallResponse};
use holochain_zome_types::{CallOutput, ExternInput};
use std::sync::Arc;

pub fn call(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CallInput,
) -> RibosomeResult<CallOutput> {
    let call = input.into_inner();
    let dna_hash = ribosome.dna_file().dna_hash().clone();
    let to_agent = call.to_agent();
    let cell_id = CellId::new(dna_hash, to_agent);
    let invocation = ZomeCallInvocation {
        cell_id,
        zome_name: call.zome_name(),
        cap: call.cap(),
        fn_name: call.fn_name(),
        payload: ExternInput::new(call.request()),
        provenance: call.provenance(),
    };
    let host_access = call_context.host_access();
    let result: ZomeCallResponse = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        let call_zome_handle = host_access.call_zome_handle();
        let workspace = host_access.workspace();
        call_zome_handle
            .call_zome(invocation, workspace)
            .await
            .map_err(Box::new)
    })??;

    Ok(CallOutput::new(result))
}

#[cfg(test)]
pub mod wasm_test {
    use std::convert::TryInto;

    use hdk3::prelude::AgentInfo;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::{ExternInput, ZomeCallResponse};
    use matches::assert_matches;

    use crate::{
        core::ribosome::ZomeCallInvocation,
        test_utils::{conductor_setup::ConductorTestData, new_invocation},
    };

    #[tokio::test(threaded_scheduler)]
    async fn call_test() {
        observability::test_run().ok();

        let zomes = vec![TestWasm::WhoAmI];
        let conductor_test = ConductorTestData::new(zomes, true).await;
        let ConductorTestData {
            __tmpdir,
            handle,
            alice_call_data,
            bob_call_data,
            ..
        } = conductor_test;
        let bob_call_data = bob_call_data.unwrap();
        let alice_cell_id = &alice_call_data.cell_id;
        let bob_cell_id = &bob_call_data.cell_id;
        let alice_agent_id = alice_cell_id.agent_pubkey();
        let bob_agent_id = bob_cell_id.agent_pubkey();

        // BOB INIT (to do cap grant)

        let _ = handle
            .call_zome(ZomeCallInvocation {
                cell_id: bob_cell_id.clone(),
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "set_access".into(),
                payload: ExternInput::new(().try_into().unwrap()),
                provenance: bob_agent_id.clone(),
            })
            .await
            .unwrap();

        // ALICE DOING A CALL

        let output = handle
            .call_zome(ZomeCallInvocation {
                cell_id: alice_cell_id.clone(),
                zome_name: TestWasm::WhoAmI.into(),
                cap: None,
                fn_name: "who_are_they_local".into(),
                payload: ExternInput::new(bob_agent_id.clone().try_into().unwrap()),
                provenance: alice_agent_id.clone(),
            })
            .await
            .unwrap()
            .unwrap();

        match output {
            ZomeCallResponse::Ok(guest_output) => {
                let response: SerializedBytes = guest_output.into_inner();
                let agent_info: AgentInfo = response.try_into().unwrap();
                assert_eq!(
                    agent_info,
                    AgentInfo {
                        agent_initial_pubkey: bob_agent_id.clone(),
                        agent_latest_pubkey: bob_agent_id.clone(),
                    },
                );
            }
            _ => unreachable!(),
        }
        ConductorTestData::shutdown_conductor(handle).await;
    }

    /// When calling the same cell we need to make sure
    /// the "as at" doesn't cause the original zome call to fail
    /// when they are both writing (moving the source chain forward)
    #[tokio::test(threaded_scheduler)]
    async fn call_the_same_cell() {
        observability::test_run().ok();

        let zomes = vec![TestWasm::WhoAmI, TestWasm::Create];
        let conductor_test = ConductorTestData::new(zomes, false).await;
        let ConductorTestData {
            __tmpdir,
            handle,
            alice_call_data,
            ..
        } = conductor_test;
        let alice_cell_id = &alice_call_data.cell_id;

        let invocation =
            new_invocation(&alice_cell_id, "call_create_entry", (), TestWasm::Create).unwrap();
        let result = handle.call_zome(invocation).await;
        assert_matches!(result, Ok(Ok(ZomeCallResponse::Ok(_))));
        ConductorTestData::shutdown_conductor(handle).await;
    }
}
