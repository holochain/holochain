#[macro_export]
macro_rules! create {
    ( $type:expr, $entry:expr ) => {{
        $crate::prelude::host_externs!(__create);
        $crate::host_fn!(
            __create,
            $crate::prelude::CreateInput::new(($type.into(), $entry.into(),)),
            $crate::prelude::CreateOutput
        )
    }};
}
