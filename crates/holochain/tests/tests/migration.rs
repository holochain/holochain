//! Tests for the chain-switch DNA migration path.
//!
//! The migration pattern works as follows:
//! 1. The migrating agent calls `prepare_migration_summary()` on the old DNA to sign the data they
//!    want to carry forward.
//! 2. They close the old chain with `close_chain_for_new(new_dna_hash)`, obtaining the real
//!    `CloseChain` action hash.
//! 3. They install the new app, passing the summary, signature, signer key, and `close_hash` as
//!    opaque `init_properties`. The new DNA also receives the trusted signer keys via DNA
//!    properties.
//! 4. During `init` on the new DNA, `get_init_properties()` reads the payload, `open_chain` is
//!    called with the real `close_hash`, and chain entries are seeded from the summary.
//! 5. The integrity zome validates the `MigrationRecord` entry, checking that the signer is listed
//!    in the DNA's `trusted_signers` and that the signature is valid.

use holo_hash::ActionHash;
use holochain::sweettest::{SweetAgents, SweetCell, SweetConductor, SweetDnaFile};
use holochain_conductor_api::CellInfo;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use std::collections::HashMap;

// Matches `migrate_initial::coordinator::MigrationSummary`
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct MigrationSummary {
    summary: Vec<String>,
    signature: Signature,
}

// Matches `migrate_new::coordinator::InitPropertiesPayload`
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct InitPropertiesPayload {
    summary: Vec<String>,
    signature: Signature,
    signer: AgentPubKey,
    close_hash: ActionHash,
}

// Matches the new definition of `MyType` in `migrate_new`
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct MyType {
    value: String,
    amount: u32,
}

/// Build the YAML DNA properties for the migration target DNA.
///
/// Hash values are encoded as their base64 string representation, which `HoloHash` can round-trip
/// through `visit_str` during msgpack deserialization in the WASM.
fn new_dna_properties(
    prev_dna_hash: &holo_hash::DnaHash,
    trusted_signers: &[AgentPubKey],
) -> SerializedBytes {
    let mut mapping = yaml_serde::Mapping::new();
    mapping.insert(
        "prev_dna_hash".into(),
        yaml_serde::Value::String(prev_dna_hash.to_string()),
    );
    mapping.insert(
        "trusted_signers".into(),
        yaml_serde::Value::Sequence(
            trusted_signers
                .iter()
                .map(|k| yaml_serde::Value::String(k.to_string()))
                .collect(),
        ),
    );
    YamlProperties::new(yaml_serde::Value::Mapping(mapping))
        .try_into()
        .unwrap()
}

/// Build a packed [`AppBundle`] from a single [`DnaFile`].
///
/// The role name and resource path are both set to the DNA hash string, which is what [`DnaFile`]'s
/// [`DnaWithRole`] impl uses by default.
fn pack_bundle(dna: &DnaFile) -> bytes::Bytes {
    let role_name = dna.dna_hash().to_string();
    AppBundle::new(
        AppManifestCurrentBuilder::default()
            .name("[generated]".into())
            .description(None)
            .roles(vec![AppRoleManifest {
                name: role_name.clone(),
                dna: AppRoleDnaManifest {
                    path: Some(role_name.clone()),
                    modifiers: DnaModifiersOpt::none(),
                    installed_hash: Some(dna.dna_hash().clone().into()),
                    clone_limit: 0,
                },
                provisioning: Some(CellProvisioning::Create { deferred: false }),
            }])
            .build()
            .unwrap()
            .into(),
        vec![(role_name, DnaBundle::from_dna_file(dna.clone()).unwrap())],
    )
    .unwrap()
    .pack()
    .unwrap()
}

/// Install and enable an app from a packed bundle, with optional per-role settings.
async fn install_and_enable(
    conductor: &SweetConductor,
    app_id: &str,
    agent: AgentPubKey,
    bundle_bytes: bytes::Bytes,
    roles_settings: Option<HashMap<RoleName, RoleSettings>>,
) {
    conductor
        .raw_handle()
        .clone()
        .install_app_bundle(InstallAppPayload {
            agent_key: Some(agent),
            source: AppBundleSource::Bytes(bundle_bytes),
            installed_app_id: Some(app_id.into()),
            network_seed: None,
            roles_settings,
            ignore_genesis_failure: false,
            restore_from_dht: false,
        })
        .await
        .unwrap();
    conductor.enable_app(app_id.to_string()).await.unwrap();
}

/// Get the single provisioned cell for an installed app.
async fn get_cell(conductor: &SweetConductor, app_id: &str, role_name: &str) -> SweetCell {
    let info = conductor
        .raw_handle()
        .get_app_info(&app_id.to_string())
        .await
        .unwrap()
        .unwrap();
    let cell_id = match info.cell_info[role_name].first().unwrap() {
        CellInfo::Provisioned(c) => c.cell_id.clone(),
        _ => panic!("Expected a provisioned cell"),
    };
    conductor.get_sweet_cell(cell_id).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_switch_migration() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let alice = SweetAgents::one(conductor.keystore()).await;

    // Install the initial DNA and create some data.
    let (old_dna, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MigrateInitial]).await;

    let old_app = conductor
        .setup_app_for_agent("app_initial", alice.clone(), std::slice::from_ref(&old_dna))
        .await
        .unwrap();
    let old_cell = old_app.into_cells().remove(0);

    let _: ActionHash = conductor
        .call(&old_cell.zome(TestWasm::MigrateInitial), "create", ())
        .await;

    // Collect the summary and sign it before closing the chain.
    let prep: MigrationSummary = conductor
        .call(
            &old_cell.zome(TestWasm::MigrateInitial),
            "prepare_migration_summary",
            (),
        )
        .await;

    // Build the new DNA with Alice as a trusted signer.
    let (new_dna, _, _) = SweetDnaFile::from_test_wasms(
        random_network_seed(),
        vec![TestWasm::MigrateNew],
        new_dna_properties(old_dna.dna_hash(), std::slice::from_ref(&alice)),
    )
    .await;

    // Close the old chain and capture the real close_hash.
    let close_hash: ActionHash = conductor
        .call(
            &old_cell.zome(TestWasm::MigrateInitial),
            "close_chain_for_new",
            new_dna.dna_hash().clone(),
        )
        .await;

    // Assemble init_properties carrying the summary, signature, signer, and close_hash.
    let payload = InitPropertiesPayload {
        summary: prep.summary.clone(),
        signature: prep.signature,
        signer: alice.clone(),
        close_hash: close_hash.clone(),
    };
    let init_props = InitProperties(SerializedBytes::try_from(&payload).unwrap());

    let role_name = new_dna.dna_hash().to_string();
    let role_settings = HashMap::from([(
        role_name.clone(),
        RoleSettings::Provisioned {
            membrane_proof: None,
            modifiers: None,
            init_properties: Some(init_props),
        },
    )]);

    install_and_enable(
        &conductor,
        "app_new",
        alice.clone(),
        pack_bundle(&new_dna),
        Some(role_settings),
    )
    .await;

    let new_cell = get_cell(&conductor, "app_new", &role_name).await;

    // Create a new entry in the new chain — this triggers init, which seeds entries from the payload.
    let _: ActionHash = conductor
        .call(&new_cell.zome(TestWasm::MigrateNew), "create", ())
        .await;

    // The new chain should contain the seeded entry (from old chain) and the newly created one.
    let results: Vec<MyType> = conductor
        .call(&new_cell.zome(TestWasm::MigrateNew), "get_all_my_types", ())
        .await;

    assert_eq!(2, results.len());
    assert_eq!(results[0].value, "test");
    assert_eq!(results[0].amount, 0);
    assert_eq!(results[1].value, "test new");
    assert_eq!(results[1].amount, 4);

    // The OpenChain action must carry the real close_hash, not a placeholder.
    let recorded_close_hash: Option<ActionHash> = conductor
        .call(
            &new_cell.zome(TestWasm::MigrateNew),
            "get_open_chain_close_hash",
            (),
        )
        .await;

    assert_eq!(
        recorded_close_hash.as_ref(),
        Some(&close_hash),
        "OpenChain.close_hash must match the real CloseChain action hash"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_switch_migration_rejects_untrusted_signer() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let alice = SweetAgents::one(conductor.keystore()).await;

    let (old_dna, _, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::MigrateInitial]).await;

    let old_app = conductor
        .setup_app_for_agent("app_initial", alice.clone(), std::slice::from_ref(&old_dna))
        .await
        .unwrap();
    let old_cell = old_app.into_cells().remove(0);

    let _: ActionHash = conductor
        .call(&old_cell.zome(TestWasm::MigrateInitial), "create", ())
        .await;

    let prep: MigrationSummary = conductor
        .call(
            &old_cell.zome(TestWasm::MigrateInitial),
            "prepare_migration_summary",
            (),
        )
        .await;

    // Build the new DNA with an EMPTY trusted_signers list, so no signer is accepted.
    let (new_dna, _, _) = SweetDnaFile::from_test_wasms(
        random_network_seed(),
        vec![TestWasm::MigrateNew],
        new_dna_properties(old_dna.dna_hash(), &[]),
    )
    .await;

    let close_hash: ActionHash = conductor
        .call(
            &old_cell.zome(TestWasm::MigrateInitial),
            "close_chain_for_new",
            new_dna.dna_hash().clone(),
        )
        .await;

    // Alice is not in trusted_signers (which is empty) — the MigrationRecord should be rejected.
    let payload = InitPropertiesPayload {
        summary: prep.summary,
        signature: prep.signature,
        signer: alice.clone(),
        close_hash,
    };
    let init_props = InitProperties(SerializedBytes::try_from(&payload).unwrap());

    let role_name = new_dna.dna_hash().to_string();
    let role_settings = HashMap::from([(
        role_name.clone(),
        RoleSettings::Provisioned {
            membrane_proof: None,
            modifiers: None,
            init_properties: Some(init_props),
        },
    )]);

    install_and_enable(
        &conductor,
        "app_new_bad",
        alice.clone(),
        pack_bundle(&new_dna),
        Some(role_settings),
    )
    .await;

    let bad_cell = get_cell(&conductor, "app_new_bad", &role_name).await;

    // The first zome call triggers init; init creates a MigrationRecord with Alice as signer, but
    // Alice is not trusted — the integrity zome must reject the commit.
    let result: Result<ActionHash, _> = conductor
        .call_fallible(&bad_cell.zome(TestWasm::MigrateNew), "create", ())
        .await;

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not listed in trusted_signers"),
        "Expected untrusted-signer rejection, got: {err}"
    );
}
