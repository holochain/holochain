use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holo_hash::HasHash;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::info::DnaInfoV1;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn dna_info_1(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfoV1, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            bindings_deterministic: Permission::Allow,
            ..
        } => Ok(DnaInfoV1 {
            name: ribosome.dna_def().name.clone(),
            hash: ribosome.dna_def().as_hash().clone(),
            properties: ribosome.dna_def().modifiers.properties.clone(),
            zome_names: ribosome
                .dna_def()
                .integrity_zomes
                .iter()
                .map(|(zome_name, _zome_def)| zome_name.to_owned())
                .collect(),
        }),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "dna_info".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod test {
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::SweetZome;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;

    async fn test_conductor(properties: SerializedBytes) -> (SweetConductor, SweetZome) {
        let (dna_file, _, _) = SweetDnaFile::from_test_wasms(
            random_network_seed(),
            vec![TestWasm::ZomeInfo],
            properties,
        )
        .await;

        let mut conductor = SweetConductor::from_standard_config().await;
        let apps = conductor.setup_apps("app-", 2, &[dna_file]).await.unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::ZomeInfo);
        let _bobbo = bobbo.zome(TestWasm::ZomeInfo);
        (conductor, alice)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dna_info_test_1() {
        holochain_trace::test_run().ok();
        // let RibosomeTestFixture {
        //     conductor, alice, ..
        // } = RibosomeTestFixture::new(TestWasm::ZomeInfo).await;

        let (conductor, alice) = test_conductor(SerializedBytes::default()).await;

        let dna_info: DnaInfoV1 = conductor.call(&alice, "dna_info_1", ()).await;
        assert_eq!(dna_info.name, String::from("Generated DnaDef"));

        let (conductor, alice) = test_conductor(SerializedBytes::default()).await;

        let dna_info: DnaInfoV1 = conductor.call(&alice, "dna_info_1", ()).await;
        assert_eq!(dna_info.name, String::from("Generated DnaDef"));
    }
}
