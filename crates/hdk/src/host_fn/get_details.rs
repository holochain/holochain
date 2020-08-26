#[macro_export]
macro_rules! get_details {
    ( $hash:expr, $options:expr ) => {{
        $crate::host_fn!(
            __get_details,
            $crate::prelude::GetDetailsInput::new(($hash.into(), $options)),
            $crate::prelude::GetDetailsOutput
        )
    }};
    ( $hash:expr ) => {
        get_details!($hash, $crate::prelude::GetOptions)
    };
}
