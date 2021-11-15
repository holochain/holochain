use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::info::DnaInfo;
use crate::core::ribosome::HostFnAccess;
use holo_hash::HasHash;
use holochain_types::prelude::*;

pub fn dna_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfo, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ bindings_deterministic: Permission::Allow, .. } => {
            Ok(DnaInfo {
                name: ribosome.dna_def().name.clone(),
                hash: ribosome.dna_def().as_hash().clone(),
                properties: ribosome.dna_def().properties.clone(),
                zome_names: ribosome
                    .dna_def()
                    .zomes
                    .iter()
                    .map(|(zome_name, _zome_def)| zome_name.to_owned())
                    .collect(),
            })
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use crate::conductor::ConductorBuilder;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use crate::core::ribosome::MockDnaStore;

    #[tokio::test(flavor = "multi_thread")]
    async fn invoke_import_dna_info_test() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ZomeInfo])
            .await
            .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store
            .expect_get_entry_def()
            .return_const(EntryDef::default_with_id("thing"));

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::ZomeInfo);
        let _bobbo = bobbo.zome(TestWasm::ZomeInfo);

        let dna_info: DnaInfo = conductor.call(&alice, "dna_info", ()).await;
        assert_eq!(dna_info.name, String::from("Generated DnaDef"));

        let dna_info_foo: serde_yaml::Value = conductor.call(&alice, "dna_info_foo", ()).await;
        assert_eq!(dna_info_foo, serde_yaml::Value::Null);

        let (dna_file_with_props, _) = SweetDnaFile::from_test_wasms(
            random_uid(),
            vec![TestWasm::ZomeInfo],
            serde_yaml::from_str("foo: bar")
        )
        .await
        .unwrap();
    }
}