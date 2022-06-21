use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

/// list all the grants stored locally in the chain filtered by tag
/// this is only the current grants as per local CRUD
pub fn capability_grants(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<(), RuntimeError> {
    unimplemented!();
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use ::fixt::prelude::*;
    use hdk::prelude::*;
    use holochain_types::fixt::CapSecretFixturator;
    use holochain_wasm_test_utils::TestWasm;

    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_capability_secret_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Capability).await;

        let _: CapSecret = conductor.call(&alice, "cap_secret", ()).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_transferable_cap_grant() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Capability).await;

        let secret: CapSecret = conductor.call(&alice, "cap_secret", ()).await;
        let action: ActionHash = conductor
            .call(&alice, "transferable_cap_grant", secret)
            .await;
        let maybe_commit: Option<Commit> = conductor.call(&alice, "get_entry", action).await;
        let entry_secret: CapSecret = maybe_commit
            .and_then(|commit| {
                let cap_grant_entry = commit.entry().to_grant_option().unwrap();
                match cap_grant_entry.access {
                    CapAccess::Transferable { secret, .. } => Some(secret),
                    _ => None,
                }
            })
            .unwrap();
        assert_eq!(entry_secret, secret);
    }

    // MAYBE: [ B-03669 ] can move this to an integration test (may need to switch to using a RibosomeStore)
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_authorized_call() -> anyhow::Result<()> {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            bob_pubkey,
            ..
        } = RibosomeTestFixture::new(TestWasm::Capability).await;

        // ALICE FAILING AN UNAUTHED CALL

        #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
        pub struct CapFor(CapSecret, AgentPubKey);

        let original_secret = CapSecretFixturator::new(Unpredictable).next().unwrap();

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_pubkey.clone().try_into().unwrap()),
            )
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        // BOB COMMITS A TRANSFERABLE GRANT WITH THE SECRET SHARED WITH ALICE

        let original_grant_hash: ActionHash = conductor
            .call(&bob, "transferable_cap_grant", original_secret)
            .await;

        // ALICE CAN NOW CALL THE AUTHED REMOTE FN

        let response: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_pubkey.clone()),
            )
            .await;

        assert_eq!(
            response,
            ZomeCallResponse::Ok(ExternIO::encode(()).unwrap()),
        );

        // BOB ROLLS THE GRANT SO ONLY THE NEW ONE WILL WORK FOR ALICE

        let new_grant_action_hash: ActionHash = conductor
            .call(&bob, "roll_cap_grant", original_grant_hash)
            .await;

        let output: Option<Commit> = conductor
            .call(&bob, "get_entry", new_grant_action_hash.clone())
            .await;

        let new_secret: CapSecret = match output {
            Some(commit) => match commit.entry().to_grant_option() {
                Some(zome_call_cap_grant) => match zome_call_cap_grant.access {
                    CapAccess::Transferable { secret, .. } => secret,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            _ => unreachable!("Couldn't get {:?}", new_grant_action_hash),
        };

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_pubkey.clone().try_into().unwrap()),
            )
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(new_secret, bob_pubkey.clone().try_into().unwrap()),
            )
            .await;
        assert_eq!(output, ZomeCallResponse::Ok(ExternIO::encode(()).unwrap()),);

        // BOB DELETES THE GRANT SO NO SECRETS WORK

        let _: ActionHash = conductor
            .call(&bob, "delete_cap_grant", new_grant_action_hash)
            .await;

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(original_secret, bob_pubkey.clone().try_into().unwrap()),
            )
            .await;

        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let output: ZomeCallResponse = conductor
            .call(
                &alice,
                "try_cap_claim",
                CapFor(new_secret, bob_pubkey.clone().try_into().unwrap()),
            )
            .await;

        // the inner response should be unauthorized
        assert_matches!(output, ZomeCallResponse::Unauthorized(_, _, _, _));

        let mut conductor = conductor;
        conductor.shutdown().await;

        Ok(())
    }
}
