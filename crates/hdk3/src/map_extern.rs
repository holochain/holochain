use crate::prelude::*;

/// Hides away the gross bit where we hook up integer pointers to length-prefixed guest memory
/// to serialization and deserialization, and returning things to the host, and memory allocation
/// and deallocation.
///
/// A lot of that is handled by the holochain_wasmer crates but this handles the boilerplate of
/// writing an extern function as they have awkward input and output signatures:
///
/// - requires remembering #[no_mangle]
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
                use $crate::prelude::*;
                use std::convert::TryFrom;

                #[no_mangle]
                pub extern "C" fn $name(guest_ptr: GuestPtr) -> GuestPtr {
                    let extern_io: ExternIO = match host_args(guest_ptr) {
                        Ok(v) => v,
                        Err(err_ptr) => return err_ptr,
                    };

                    let inner: $input = match extern_io.decode() {
                        Ok(v) => v,
                        Err(_) => return return_err_ptr(WasmError::Deserialize(vec![0])),
                    };

                    let output: $output = match super::$f(inner) {
                        Ok(v) => Ok(v),
                        Err(wasm_error) => return return_err_ptr(wasm_error),
                    };

                    match ExternIO::encode(output.unwrap()) {
                        Ok(v) => return_ptr::<ExternIO>(v),
                        Err(serialized_bytes_error) => return_err_ptr(WasmError::Serialize(serialized_bytes_error)),
                    }
                }
            }
        }
    };
}

pub type ExternResult<T> = Result<T, WasmError>;
