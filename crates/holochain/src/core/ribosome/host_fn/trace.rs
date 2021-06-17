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
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_state::host_fn_workspace::HostFnWorkspace;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;

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
        let test_env = holochain_state::test_utils::test_cell_env();
        let test_cache = holochain_state::test_utils::test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();
        crate::test_utils::fake_genesis(env.clone())
            .await
            .unwrap();
        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author).await.unwrap();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace;

        // this shows that we can get line numbers out of wasm
        let output: () = crate::call_test_ribosome!(host_access, TestWasm::Debug, "debug", ());
        assert_eq!(output, ());
    }
}
