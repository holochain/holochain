#[macro_export]
macro_rules! delete_link {
    ( $add_link_header:expr ) => {{
        $crate::prelude::host_externs!(__delete_link);

        $crate::host_fn!(
            __delete_link,
            $crate::prelude::DeleteLinkInput::new($add_link_header),
            $crate::prelude::DeleteLinkOutput
        )
    }};
}
