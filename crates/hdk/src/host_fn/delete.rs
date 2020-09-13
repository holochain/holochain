#[macro_export]
macro_rules! delete {
    ( $hash:expr ) => {{
        $crate::prelude::host_externs!(__delete);

        $crate::host_fn!(
            __delete,
            $crate::prelude::DeleteInput::new($hash.into()),
            $crate::prelude::DeleteOutput
        )
    }};
}
