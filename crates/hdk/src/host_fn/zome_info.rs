/// Trivial macro to get the zome information.
/// There are no inputs to zome_info.
///
/// Zome information includes dna name, hash, zome name and properties.
///
/// In general any holochain compatible wasm can be compiled and run in any zome so the zome info
/// needs to be looked up at runtime to e.g. know where to send/receive call_remote rpc calls to.
#[macro_export]
macro_rules! zome_info {
    () => {{
        $crate::host_fn!(
            __zome_info,
            $crate::prelude::ZomeInfoInput::new(()),
            $crate::prelude::ZomeInfoOutput
        )
    }};
}
