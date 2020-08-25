#[macro_export]
macro_rules! random_bytes {
    ( $bytes:expr ) => {{
        $crate::host_fn!(
            __random_bytes,
            $crate::prelude::RandomBytesInput::new($bytes),
            $crate::prelude::RandomBytesOutput
        )
    }};
}
