#[macro_export]
macro_rules! delete_entry {
    ( $hash:expr ) => {{
        $crate::host_fn!(
            __delete_entry,
            $crate::prelude::DeleteEntryInput::new(($hash.into())),
            $crate::prelude::DeleteEntryOutput
        )
    }};
}
