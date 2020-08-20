#[macro_export]
macro_rules! get {
    ( $hash:expr, $options:expr ) => {{
        $crate::host_fn!(
            __get,
            $crate::prelude::GetInput::new(($hash.into(), $options)),
            $crate::prelude::GetOutput
        )
    }};
    ( $input:expr ) => {
        get!($input, $crate::prelude::GetOptions)
    };
}
