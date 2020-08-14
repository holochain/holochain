#[macro_export]
macro_rules! remove_link {
    ( $add_link_header:expr ) => {{
        $crate::host_fn!(
            __remove_link,
            $crate::prelude::RemoveLinkInput::new($add_link_header),
            $crate::prelude::RemoveLinkOutput
        )
    }};
}
