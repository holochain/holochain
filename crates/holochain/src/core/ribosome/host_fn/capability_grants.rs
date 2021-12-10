use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

/// list all the grants stored locally in the chain filtered by tag
/// this is only the current grants as per local CRUD
pub fn capability_grants(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), WasmError> {
    unimplemented!();
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::sweettest::SweetDnaFile;
    use crate::{conductor::ConductorBuilder, sweettest::SweetConductor};
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_types::fixt::CapSecretFixturator;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_agent_pubkey_2;
    use holochain_wasm_test_utils::TestWasm;

    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_capability_secret_test<'a>() {
        observability::test_run().ok();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let _: CapSecret =
            crate::call_test_ribosome!(host_access, TestWasm::Capability, "cap_secret", ())
                .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_transferable_cap_grant<'a>() {
        observability::test_run().ok();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let secret: CapSecret =
            crate::call_test_ribosome!(host_access, TestWasm::Capability, "cap_secret", ())
                .unwrap();
        let header: HeaderHash = crate::call_test_ribosome!(
            host_access,
            TestWasm::Capability,
            "transferable_cap_grant",
            secret
        )
        .unwrap();
        let maybe_element: Option<Element> =
            crate::call_test_ribosome!(host_access, TestWasm::Capability, "get_entry", header)
                .unwrap();

        let entry_secret: CapSecret = match maybe_element {
            Some(element) => {
                let cap_grant_entry: CapGrantEntry = element.entry().to_grant_option().unwrap();
                match cap_grant_entry.access {
                    CapAccess::Transferable { secret, .. } => secret,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        };
        assert_eq!(entry_secret, secret,);
    }

    // MAYBE: [ B-03669 ] can move this to an integration test (may need to switch to using a RealDnaStore)
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_authorized_call() -> anyhow::Result<()> {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Capability])
            .await
            .unwrap();

        let alice_agent_id = fake_agent_pubkey_1();
        let bob_agent_id = fake_agent_pubkey_2();

        let mut dna_store = MockDnaStore::new();
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store.expect_add_dna().return_const(());
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_agent_id.clone(), bob_agent_id.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        // There's only one zome to call, so let's peel that off now.
        let alice = alice.zome(TestWasm::Capability);
        let bobbo = bobbo.zome(TestWasm::Capability);

        // ALICE FAILING AN UNAUTHED CALL

        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
        pub struct CapFor(CapSecret, AgentPubKey);

        let original_secret = CapSecretFixturator::new(Unpredictable).next().unwrap();

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_agent_id.clone().try_into().unwrap()),
            )
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        // BOB COMMITS A TRANSFERABLE GRANT WITH THE SECRET SHARED WITH ALICE

        let original_grant_hash: HeaderHash = conductor
            .call(&bobbo, "transferable_cap_grant", original_secret)
            .await;

        // ALICE CAN NOW CALL THE AUTHED REMOTE FN

        let response: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_agent_id.clone()),
            )
            .await;

        assert_eq!(
            response,
            ZomeCallResponse::Ok(ExternIO::encode(()).unwrap()),
        );

        // BOB ROLLS THE GRANT SO ONLY THE NEW ONE WILL WORK FOR ALICE

        let new_grant_header_hash: HeaderHash = conductor
            .call(&bobbo, "roll_cap_grant", original_grant_hash)
            .await;

        let output: Option<Element> = conductor
            .call(&bobbo, "get_entry", new_grant_header_hash.clone())
            .await;

        let new_secret: CapSecret = match output {
            Some(element) => match element.entry().to_grant_option() {
                Some(zome_call_cap_grant) => match zome_call_cap_grant.access {
                    CapAccess::Transferable { secret, .. } => secret,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            _ => unreachable!("Couldn't get {:?}", new_grant_header_hash),
        };

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_agent_id.clone().try_into().unwrap()),
            )
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(new_secret, bob_agent_id.clone().try_into().unwrap()),
            )
            .await;
        assert_eq!(output, ZomeCallResponse::Ok(ExternIO::encode(()).unwrap()),);

        // BOB DELETES THE GRANT SO NO SECRETS WORK

        let _: HeaderHash = conductor
            .call(&bobbo, "delete_cap_grant", new_grant_header_hash)
            .await;

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_agent_id.clone().try_into().unwrap()),
            )
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(new_secret, bob_agent_id.clone().try_into().unwrap()),
            )
            .await;

        // the inner response should be unauthorized
        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let mut conductor = conductor;
        conductor.shutdown().await;

        Ok(())
    }
}
