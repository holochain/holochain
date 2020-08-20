#[macro_export]
macro_rules! get_links {
    ( $base:expr ) => {
        $crate::get_links!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::host_fn!(
            __get_links,
            $crate::prelude::GetLinksInput::new(($base, $tag.into())),
            $crate::prelude::GetLinksOutput
        )
    }};
}
