#[macro_export]
macro_rules! link_entries {
    ( $base:expr, $target:expr ) => {
        $crate::link_entries!($base, $target, vec![])
    };
    ( $base:expr, $target:expr, $tag:expr ) => {{
        $crate::host_fn!(
            __link_entries,
            $crate::prelude::LinkEntriesInput::new(($base, $target, $tag.into())),
            $crate::prelude::LinkEntriesOutput
        )
    }};
}
