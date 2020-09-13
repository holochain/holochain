#[macro_export]
macro_rules! create_link {
    ( $base:expr, $target:expr ) => {
        $crate::create_link!($base, $target, vec![])
    };
    ( $base:expr, $target:expr, $tag:expr ) => {{
        $crate::prelude::host_externs!(__create_link);

        $crate::host_fn!(
            __create_link,
            $crate::prelude::CreateLinkInput::new(($base, $target, $tag.into())),
            $crate::prelude::CreateLinkOutput
        )
    }};
}
