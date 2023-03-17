use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use once_cell::unsync::Lazy;
use std::sync::Arc;
use tracing::*;

#[cfg(test)]
use once_cell::sync::Lazy as SyncLazy;
#[cfg(test)]
use std::sync::atomic::AtomicBool;

#[cfg(test)]
static CAPTURE: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static CAPTURED: SyncLazy<Arc<std::sync::Mutex<Vec<TraceMsg>>>> =
    SyncLazy::new(|| Arc::new(std::sync::Mutex::new(Vec::new())));

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
) -> Result<(), RuntimeError> {
    // Avoid dialing out to the environment on every trace.
    let wasm_log = Lazy::new(|| {
        std::env::var("WASM_LOG").unwrap_or_else(|_| "[wasm_trace]=debug".to_string())
    });
    let collector = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new((*wasm_log).clone()))
        .with_target(false)
        .finish();

    #[cfg(test)]
    if CAPTURE.load(std::sync::atomic::Ordering::Relaxed) {
        CAPTURED.lock().unwrap().push(input.clone());
    }

    tracing::subscriber::with_default(collector, || wasm_trace(input));
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::*;

    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::fixt::CallContextFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;
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
            level: holochain_types::prelude::Level::DEBUG,
            msg: "ribosome trace works".to_string(),
        };

        let output: () = trace(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!((), output);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "Doesn't work concurrently"]
    async fn wasm_trace_test() {
        use holochain_types::prelude::Level::*;
        CAPTURE.store(true, std::sync::atomic::Ordering::SeqCst);
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Debug).await;

        let _: () = conductor.call(&alice, "debug", ()).await;
        let r: Vec<_> = CAPTURED.lock().unwrap().clone();
        let expect = vec![
            // two traces from the two validations of genesis entries
            TraceMsg {
                msg:
                    "integrity_test_wasm_debug:debug/src/integrity.rs:5 tracing in validation works"
                        .to_string(),
                level: INFO,
            },
            TraceMsg {
                msg:
                    "integrity_test_wasm_debug:debug/src/integrity.rs:5 tracing in validation works"
                        .to_string(),
                level: INFO,
            },
            // followed by the zome call traces
            TraceMsg {
                msg: "test_wasm_debug:debug/src/lib.rs:5 tracing works!".to_string(),
                level: TRACE,
            },
            TraceMsg {
                msg: "test_wasm_debug:debug/src/lib.rs:6 debug works".to_string(),
                level: DEBUG,
            },
            TraceMsg {
                msg: "test_wasm_debug:debug/src/lib.rs:7 info works".to_string(),
                level: INFO,
            },
            TraceMsg {
                msg: "test_wasm_debug:debug/src/lib.rs:8 warn works".to_string(),
                level: WARN,
            },
            TraceMsg {
                msg: "test_wasm_debug:debug/src/lib.rs:9 error works".to_string(),
                level: ERROR,
            },
            TraceMsg {
                msg: "test_wasm_debug:debug/src/lib.rs:10 foo = \"fields\"; bar = \"work\"; too"
                    .to_string(),
                level: DEBUG,
            },
        ];
        assert_eq!(r, expect);
    }
}
