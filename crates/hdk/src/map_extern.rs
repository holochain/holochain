use crate::prelude::*;

#[macro_export]
macro_rules! map_extern_preamble {
    ( $guest_ptr:ident, $len:ident, $inner:ident, $input:ty, $output:ty ) => {
        // Setup tracing.
        // @TODO feature flag this?
        let _subscriber_guard = $crate::prelude::tracing::subscriber::set_default(
            $crate::trace::WasmSubscriber::default()
        );

        // Deserialize the input from the host.
        let extern_io: $crate::prelude::ExternIO = match $crate::prelude::host_args($guest_ptr, $len) {
            Ok(v) => v,
            Err(err_ptr) => return err_ptr,
        };
        let $inner: $input = match extern_io.decode() {
            Ok(v) => v,
            Err(e) => {
                let bytes = extern_io.0;
                $crate::prelude::error!(output_type = std::any::type_name::<$output>(), bytes = ?bytes, "{}", e);
                return $crate::prelude::return_err_ptr($crate::prelude::wasm_error!($crate::prelude::WasmErrorInner::Deserialize(bytes)));
            }
        };
    }
}

pub fn encode_to_guestptrlen<T: std::fmt::Debug + Serialize>(v: T) -> GuestPtrLen {
    match ExternIO::encode(v) {
        Ok(v) => return_ptr::<ExternIO>(v),
        Err(serialized_bytes_error) => return_err_ptr(wasm_error!(WasmErrorInner::Serialize(
            serialized_bytes_error
        ))),
    }
}

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
                    $crate::map_extern_preamble!(guest_ptr, len, inner, $input, $output);
                    match super::$f(inner) {
                        Ok(v) => $crate::map_extern::encode_to_guestptrlen(v),
                        Err(e) => $crate::prelude::return_err_ptr(e),
                    }
                }
            }
        }
    };
}

#[macro_export]
macro_rules! map_extern_infallible {
    ( $name:tt, $f:ident, $input:ty, $output:ty ) => {
        $crate::paste::paste! {
            mod [< __ $name _extern >] {
                use super::*;

                #[no_mangle]
                pub extern "C" fn $name(guest_ptr: $crate::prelude::GuestPtr, len: $crate::prelude::Len) -> $crate::prelude::GuestPtrLen {
                    $crate::map_extern_preamble!(guest_ptr, len, inner, $input, $output);
                    $crate::map_extern::encode_to_guestptrlen(super::$f(inner))
                }
            }
        }
    }
}

/// Every extern _must_ retern a `WasmError` in the case of failure.
pub type ExternResult<T> = Result<T, WasmError>;
