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
    ( $name:tt, $f:ident ) => {
        #[no_mangle]
        pub extern "C" fn $name(ptr: $crate::prelude::GuestPtr) -> $crate::prelude::GuestPtr {
            let input: $crate::prelude::ExternInput = $crate::prelude::host_args!(ptr);
            let result = $f($crate::prelude::try_result!(
                input.into_inner().try_into(),
                "failed to deserialize args"
            ));
            let result_value = $crate::prelude::try_result!(
                result,
                // TODO: this name includes "_hdk_extern", should we strip it off?
                concat!("inner function '", stringify!($f), "' failed")
            );
            let result_sb = $crate::prelude::try_result!(
                $crate::prelude::SerializedBytes::try_from(result_value),
                "inner function result serialization error"
            );
            $crate::prelude::ret!($crate::prelude::ExternOutput::new(result_sb));
        }
    };
}

pub type ExternResult<T> = Result<T, crate::prelude::HdkError>;
