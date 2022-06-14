use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeT;
use holo_hash::HasHash;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::info::DnaInfo;
use std::sync::Arc;
use crate::core::ribosome::RibosomeError;

pub fn dna_info(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<DnaInfo, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            bindings_deterministic: Permission::Allow,
            ..
        } => Ok(DnaInfo {
            name: ribosome.dna_def().name.clone(),
            hash: ribosome.dna_def().as_hash().clone(),
            properties: ribosome.dna_def().properties.clone(),
            zome_names: ribosome
                .dna_def()
                .zomes
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
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;

    async fn test_conductor(properties: SerializedBytes) -> (SweetConductor, SweetZome) {
        let (dna_file, _) =
            SweetDnaFile::from_test_wasms(random_uid(), vec![TestWasm::ZomeInfo], properties)
                .await
                .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut conductor = SweetConductor::from_standard_config().await;
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
        (conductor, alice)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn dna_info_test() {
        observability::test_run().ok();
        // let RibosomeTestFixture {
        //     conductor, alice, ..
        // } = RibosomeTestFixture::new(TestWasm::ZomeInfo).await;

        let (conductor, alice) = test_conductor(SerializedBytes::default()).await;

        let dna_info: DnaInfo = conductor.call(&alice, "dna_info", ()).await;
        assert_eq!(dna_info.name, String::from("Generated DnaDef"));

        let (conductor, alice) = test_conductor(SerializedBytes::default()).await;

        let dna_info: DnaInfo = conductor.call(&alice, "dna_info", ()).await;
        assert_eq!(dna_info.name, String::from("Generated DnaDef"));

        let dna_info_foo: Option<String> = conductor.call(&alice, "dna_info_value", "foo").await;
        assert_eq!(dna_info_foo, None);
        let dna_info_foo_direct: Option<String> =
            conductor.call(&alice, "dna_info_foo_direct", ()).await;
        assert_eq!(dna_info_foo_direct, None);

        let dna_info_bar: Option<String> = conductor.call(&alice, "dna_info_value", "bar").await;
        assert_eq!(dna_info_bar, None);
        let dna_info_bar_direct: Option<String> =
            conductor.call(&alice, "dna_info_bar_direct", ()).await;
        assert_eq!(dna_info_bar_direct, None);

        let yaml = "foo: bar";
        let (conductor, alice) = test_conductor(
            YamlProperties::new(serde_yaml::from_str(yaml).unwrap())
                .try_into()
                .unwrap(),
        )
        .await;
        let dna_info_foo: Option<String> = conductor.call(&alice, "dna_info_value", "foo").await;
        assert_eq!(dna_info_foo, Some("bar".into()));
        let dna_info_foo_direct: Option<String> =
            conductor.call(&alice, "dna_info_foo_direct", ()).await;
        assert_eq!(dna_info_foo_direct, Some("bar".into()));

        let dna_info_bar: Option<String> = conductor.call(&alice, "dna_info_value", "bar").await;
        assert_eq!(dna_info_bar, None);
        let dna_info_bar_direct: Option<String> =
            conductor.call(&alice, "dna_info_bar_direct", ()).await;
        assert_eq!(dna_info_bar_direct, None);

        let yaml = "foo: 1\nbar: bing";
        let (conductor, alice) = test_conductor(
            YamlProperties::new(serde_yaml::from_str(yaml).unwrap())
                .try_into()
                .unwrap(),
        )
        .await;
        let dna_info_foo: Option<u64> = conductor.call(&alice, "dna_info_value", "foo").await;
        assert_eq!(dna_info_foo, Some(1));
        let dna_info_foo_direct: Option<u64> =
            conductor.call(&alice, "dna_info_foo_direct", ()).await;
        assert_eq!(dna_info_foo_direct, Some(1));

        let dna_info_bar: Option<String> = conductor.call(&alice, "dna_info_value", "bar").await;
        assert_eq!(dna_info_bar, Some("bing".into()));
        let dna_info_bar_direct: Option<String> =
            conductor.call(&alice, "dna_info_bar_direct", ()).await;
        assert_eq!(dna_info_bar_direct, Some("bing".into()));

        let yaml = "baz: \n  foo: \n   bar: 1";
        let (conductor, alice) = test_conductor(
            YamlProperties::new(serde_yaml::from_str(yaml).unwrap())
                .try_into()
                .unwrap(),
        )
        .await;
        let nested: Option<i64> = conductor.call(&alice, "dna_info_nested", ()).await;
        assert_eq!(nested, Some(1));
    }
}
