/// trivial wrapper around the __delete_entry host_fn
/// takes any expression that evaluates to an entry hash
///
/// ```ignore
/// delete_entry!(entry_hash!(foo_entry)?)?;
/// ```
#[macro_export]
macro_rules! delete_entry {
    ( $hash:expr ) => {{
        $crate::host_fn!(
            __delete_entry,
            $crate::prelude::DeleteEntryInput::new($hash.into()),
            $crate::prelude::DeleteEntryOutput
        )
    }};
}
