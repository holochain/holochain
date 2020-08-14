#[macro_export]
macro_rules! map_extern {
    ( $name:tt, $f:ident ) => {
        #[no_mangle]
        pub extern "C" fn $name(ptr: $crate::prelude::GuestPtr) -> $crate::prelude::GuestPtr {
            let input: $crate::prelude::HostInput = $crate::prelude::host_args!(ptr);
            let result = $f($crate::prelude::try_result!(
                input.into_inner().try_into(),
                "failed to deserialize args"
            ));
            let result_value = $crate::prelude::try_result!(result, "inner function failed");
            let result_sb = $crate::prelude::try_result!(
                $crate::prelude::SerializedBytes::try_from(result_value),
                "inner function result serialization error"
            );
            $crate::prelude::ret!($crate::prelude::GuestOutput::new(result_sb));
        }
    };
}
