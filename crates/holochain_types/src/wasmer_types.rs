//! These types manage the consumption and configuration of the `wasmer` library
//! in terms of middleware (metering), Target, Module, and Store.

use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
use wasmer::{
    wasmparser, CompileError, CompilerConfig, CpuFeature, Cranelift, Engine, Module, Store, Target,
    Triple,
};
use wasmer_middlewares::*;

#[cfg(not(test))]
/// one hundred giga ops
pub const WASM_METERING_LIMIT: u64 = 100_000_000_000;

#[cfg(test)]
/// ten mega ops.
/// We don't want tests to run forever, and it can take several minutes for 100 giga ops to run.
pub const WASM_METERING_LIMIT: u64 = 10_000_000;

/// Generate a Cranelift type (1 of 3 possible types) wasm compiler
/// with Metering (use limits) in place.
pub fn cranelift() -> Engine {
    let cost_function = |_operator: &wasmparser::Operator| -> u64 { 1 };
    // @todo 100 giga-ops is totally arbitrary cutoff so we probably
    // want to make the limit configurable somehow.
    let metering = Arc::new(Metering::new(WASM_METERING_LIMIT, cost_function));
    let mut compiler = Cranelift::default();
    compiler.canonicalize_nans(true).push_middleware(metering);
    Engine::from(compiler)
}

/// Configuration of a Target for wasmer for iOS
pub fn wasmer_ios_target() -> Target {
    // use what I see in
    // platform ios headless example
    // https://github.com/wasmerio/wasmer/blob/447c2e3a152438db67be9ef649327fabcad6f5b8/examples/platform_ios_headless.rs#L38-L53
    let triple = Triple::from_str("aarch64-apple-ios").unwrap();
    let cpu_feature = CpuFeature::set();
    Target::new(triple, cpu_feature)
}

/// Take WASM binary and prepare a wasmer Module suitable for iOS
pub fn build_ios_module(wasm: &[u8]) -> Result<Module, CompileError> {
    info!(
        "Found wasm and was instructed to serialize it for ios in wasmer format, doing so now..."
    );
    let compiler_config = cranelift();
    let store = Store::new(compiler_config);
    Module::from_binary(&store, wasm)
}

/// Generate a Dylib Engine suitable for iOS.
/// Useful for re-building an iOS Module from a preserialized WASM Module.
pub fn ios_dylib_headless_engine() -> Engine {
    Engine::default()
}
