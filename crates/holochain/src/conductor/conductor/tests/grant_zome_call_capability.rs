use crate::conductor::api::error::ConductorApiError;
use crate::conductor::error::ConductorError;
use crate::sweettest::{SweetConductor, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::{
    CapGrant, Capability, GrantConstraint, GrantZomeCallCapabilityPayload, GrantedFunctions,
    ZomeCallGrant,
};
use matches::assert_matches;

/// The admin call issues zome call grants only. A grant for any other capability is
/// rejected rather than committed as an entry that could never authorize a zome call.
#[tokio::test(flavor = "multi_thread")]
async fn rejects_a_non_zome_call_capability() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("app", [&dna]).await.unwrap();
    let cell_id = app.cells()[0].cell_id().clone();

    let err = conductor
        .raw_handle()
        .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id: cell_id.clone(),
            cap_grant: CapGrant {
                tag: "signals".into(),
                constraint: GrantConstraint::Unrestricted,
                capability: Capability::DirectSignal,
            },
        })
        .await
        .unwrap_err();

    assert_matches!(
        err,
        ConductorApiError::ConductorError(ConductorError::NotAZomeCallGrant(_))
    );

    // A zome call grant on the same cell still succeeds.
    conductor
        .raw_handle()
        .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id,
            cap_grant: CapGrant {
                tag: "calls".into(),
                constraint: GrantConstraint::Unrestricted,
                capability: Capability::ZomeCall(ZomeCallGrant {
                    functions: GrantedFunctions::All,
                }),
            },
        })
        .await
        .unwrap();
}
