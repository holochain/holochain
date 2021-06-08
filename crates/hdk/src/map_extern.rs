use crate::prelude::*;

/// Hides away the gross bit where we hook up integer pointers to length-prefixed guest memory
/// to serialization and deserialization, and returning things to the host, and memory allocation
/// and deallocation.
///
/// A lot of that is handled by the holochain_wasmer crates but this handles the boilerplate of
/// writing an extern function as they have awkward input and output signatures:
///
/// - requires remembering `#[no_mangle]`
/// - requires remembering pub extern "C"
/// - requires juggling GuestPtr on the input and output with the memory/serialization
/// - doesn't support Result returns at all, so breaks things as simple as `?`
///
/// This can be used directly as `map_extern!(external_facing_fn_name, internal_fn_name)` but it is
/// more idiomatic to use the `#[hdk_extern]` attribute
///
/// ```ignore
/// #[hdk_extern]
/// pub fn foo(foo_input: FooInput) -> ExternResult<FooOutput> {
///  // ... do stuff to respond to incoming calls from the host to "foo"
/// }
/// ```
#[macro_export]
macro_rules! map_extern {
    ( $name:tt, $f:ident, $input:ty, $output:ty ) => {
        $crate::paste::paste! {
            mod [< __ $name _extern >] {
                use super::*;

                #[no_mangle]
                pub extern "C" fn $name(guest_ptr: $crate::prelude::GuestPtr, len: $crate::prelude::Len) -> $crate::prelude::GuestPtrLen {
                    // Setup tracing.
                    // @TODO feature flag this?
                    let _subscriber_guard = $crate::prelude::tracing::subscriber::set_default(
                        $crate::trace::WasmSubscriber::default()
                    );

                    // Deserialize the input from the host.
                    let extern_io: $crate::prelude::ExternIO = match $crate::prelude::host_args(guest_ptr, len) {
                        Ok(v) => v,
                        Err(err_ptr) => return err_ptr,
                    };
                    let inner: $input = match extern_io.decode() {
                        Ok(v) => v,
                        Err(e) => {
                            let bytes = extern_io.0;
                            $crate::prelude::error!(output_type = std::any::type_name::<$output>(), bytes = ?bytes, "{}", e);
                            return $crate::prelude::return_err_ptr($crate::prelude::WasmError::Deserialize(bytes));
                        }
                    };

                    // Call the function and handle the output.
                    let maybe_extern_io: std::result::Result<$crate::prelude::ExternIO, $crate::prelude::SerializedBytesError> = match super::$f(inner) {
                        Ok(v) => {
                            $crate::prelude::ExternIO::encode(v)
                        },
                        Err(e) => {
                            let output_type_id = std::any::TypeId::of::<$output>();
                            // Callback results have a pass/fail nature that needs to map some wasm errors to an Ok(XCallbackResult::Fail).
                            if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::EntryDefsCallbackResult>>() {
                                match $crate::prelude::EntryDefsCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            }
                            else if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::InitCallbackResult>>() {
                                match $crate::prelude::InitCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            } else if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::MigrateAgentCallbackResult>>() {
                                match $crate::prelude::MigrateAgentCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            }
                            else if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::PostCommitCallbackResult>>() {
                                match $crate::prelude::PostCommitCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            }
                            else if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::ValidateLinkCallbackResult>>() {
                                match $crate::prelude::ValidateLinkCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            }
                            else if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::ValidateCallbackResult>>() {
                                match $crate::prelude::ValidateCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            } else if output_type_id == std::any::TypeId::of::<$crate::prelude::ExternResult<$crate::prelude::ValidationPackageCallbackResult>>() {
                                match $crate::prelude::ValidationPackageCallbackResult::try_from_wasm_error(e) {
                                    Ok(v) => $crate::prelude::ExternIO::encode(v),
                                    Err(e) => return $crate::prelude::return_err_ptr(e),
                                }
                            }
                            else {
                                return $crate::prelude::return_err_ptr(e);
                            }
                        },
                    };
                    match maybe_extern_io {
                        Ok(v) => $crate::prelude::return_ptr::<$crate::prelude::ExternIO>(v),
                        Err(serialized_bytes_error) => $crate::prelude::return_err_ptr($crate::prelude::WasmError::Serialize(serialized_bytes_error)),
                    }
                }
            }
        }
    };
}

/// Every extern _must_ retern a `WasmError` in the case of failure.
pub type ExternResult<T> = Result<T, WasmError>;
