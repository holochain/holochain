#[macro_export]
macro_rules! get_link_details {
    ( $base:expr ) => {
        $crate::get_link_details!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::host_fn!(
            __get_link_details,
            GetLinkDetailsInput::new(($base, $tag.into())),
            GetLinkDetailsOutput
        )
    }};
}
