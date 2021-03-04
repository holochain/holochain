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
        map_extern!($name, $f, $input, $output, true);
    };
    ( $name:tt, $f:ident, $input:ty, $output:ty, false) => {
        map_extern!($name, $f, $input, $output: init = {};);
    };
    ( $name:tt, $f:ident, $input:ty, $output:ty, true ) => {
        map_extern!($name, $f, $input, $output: init = {
            match $crate::prelude::set_global_hdk($crate::prelude::HostHdk) {
                Ok(_) => {},
                Err(_) => return $crate::prelude::return_err_ptr($crate::prelude::WasmError::Guest("Failed to set the global HDK".to_string())),
            }
        };);
    };
    ( $name:tt, $f:ident, $input:ty, $output:ty: init = $init_hdk:expr; ) => {
        $crate::paste::paste! {
            mod [< __ $name _extern >] {
                use super::*;

                #[no_mangle]
                pub extern "C" fn $name(guest_ptr: $crate::prelude::GuestPtr) -> $crate::prelude::GuestPtr {
                    $init_hdk;

                    // Setup tracing.
                    // @TODO feature flag this?
                    match $crate::prelude::tracing::subscriber::set_global_default(
                        $crate::trace::WasmSubscriber::default()
                    ) {
                        Ok(_) => {},
                        Err(e) => return $crate::prelude::return_err_ptr($crate::prelude::WasmError::Guest(e.to_string())),
                    }

                    // Deserialize the input from the host.
                    let extern_io: $crate::prelude::ExternIO = match $crate::prelude::host_args(guest_ptr) {
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

                    // Call the function.
                    let output: $output = match super::$f(inner) {
                        Ok(v) => Ok(v),
                        Err(wasm_error) => return $crate::prelude::return_err_ptr(wasm_error),
                    };

                    // Serialize the output for the host.
                    match $crate::prelude::ExternIO::encode(output.unwrap()) {
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
