//! These types manage the consumption and configuration of the `wasmer` library
//! in terms of middleware (metering), Target, Module, and Store.
//!
//! Note that this module is deprecated. Use https://github.com/holochain/holochain-wasmer/blob/develop/crates/host/src/module.rs#L135-L185 instead.

use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
use wasmer::{
    wasmparser, CompileError, CompilerConfig, CpuFeature, Cranelift, Engine, Module,
    NativeEngineExt, Store, Target, Triple,
};
use wasmer_middlewares::*;

#[cfg(not(test))]
/// one hundred giga ops
pub const WASM_METERING_LIMIT: u64 = 100_000_000_000;

#[cfg(test)]
/// ten mega ops.
/// We don't want tests to run forever, and it can take several minutes for 100 giga ops to run.
#[deprecated]
pub const WASM_METERING_LIMIT: u64 = 10_000_000;

/// Generate a Cranelift type (1 of 3 possible types) wasm compiler
/// with Metering (use limits) in place.
#[deprecated]
pub fn cranelift() -> Cranelift {
    let cost_function = |_operator: &wasmparser::Operator| -> u64 { 1 };
    // @todo 100 giga-ops is totally arbitrary cutoff so we probably
    // want to make the limit configurable somehow.
    let metering = Arc::new(Metering::new(WASM_METERING_LIMIT, cost_function));
    let mut cranelift = Cranelift::default();
    cranelift.canonicalize_nans(true).push_middleware(metering);
    cranelift
}

/// Configuration of a Target for wasmer for iOS
#[deprecated]
pub fn wasmer_ios_target() -> Target {
    // use what I see in
    // platform ios headless example
    // https://github.com/wasmerio/wasmer/blob/447c2e3a152438db67be9ef649327fabcad6f5b8/examples/platform_ios_headless.rs#L38-L53
    let triple = Triple::from_str("aarch64-apple-ios").unwrap();
    let cpu_feature = CpuFeature::set();
    Target::new(triple, cpu_feature)
}

/// Take WASM binary and prepare a wasmer Module suitable for iOS
#[deprecated]
pub fn build_ios_module(wasm: &[u8]) -> Result<Module, CompileError> {
    info!(
        "Found wasm and was instructed to serialize it for ios in wasmer format, doing so now..."
    );
    let compiler_config = cranelift();
    let engine = Engine::from(compiler_config);
    let store = Store::new(engine);
    Module::from_binary(&store, wasm)
}

/// Generate a headless Dylib Store suitable for iOS.
/// Useful for re-building an iOS Module from a preserialized WASM Module.
#[deprecated]
pub fn ios_dylib_headless_store() -> Store {
    let engine = Engine::headless();
    Store::new(engine)
}
