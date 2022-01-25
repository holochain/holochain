use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use once_cell::unsync::Lazy;
use std::sync::Arc;
use tracing::*;

#[instrument(skip(input))]
pub fn wasm_trace(input: TraceMsg) {
    match input.level {
        holochain_types::prelude::Level::TRACE => tracing::trace!("{}", input.msg),
        holochain_types::prelude::Level::DEBUG => tracing::debug!("{}", input.msg),
        holochain_types::prelude::Level::INFO => tracing::info!("{}", input.msg),
        holochain_types::prelude::Level::WARN => tracing::warn!("{}", input.msg),
        holochain_types::prelude::Level::ERROR => tracing::error!("{}", input.msg),
    }
}

pub fn trace(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: TraceMsg,
) -> Result<(), WasmError> {
    // Avoid dialing out to the environment on every trace.
    let wasm_log = Lazy::new(|| {
        std::env::var("WASM_LOG").unwrap_or_else(|_| "[wasm_trace]=debug".to_string())
    });
    let collector = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new((*wasm_log).clone()))
        .with_target(false)
        .finish();
    tracing::subscriber::with_default(collector, || wasm_trace(input));
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::trace;

    use crate::fixt::CallContextFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;
    use crate::sweettest::SweetDnaFile;
    use crate::core::ribosome::MockDnaStore;
    use crate::conductor::ConductorBuilder;
    use crate::sweettest::SweetConductor;

    /// we can get an entry hash out of the fn directly
    #[tokio::test(flavor = "multi_thread")]
    async fn trace_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let input = TraceMsg {
            level: Level::DEBUG,
            msg: "ribosome trace works".to_string(),
        };

        let output: () = trace(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!((), output);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wasm_trace_test() {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Debug])
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
        let alice = alice.zome(TestWasm::Debug);
        let _bobbo = bobbo.zome(TestWasm::Debug);

        let _: () = conductor.call(&alice, "debug", ()).await;
    }
}