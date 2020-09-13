#[macro_export]
macro_rules! update {
    ( $hash:expr, $type:expr, $input:expr ) => {{
        $crate::prelude::host_externs!(__update);

        $crate::host_fn!(
            __update,
            $crate::prelude::UpdateInput::new(($type, $input, $hash)),
            $crate::prelude::UpdateOutput
        )
    }};
}
