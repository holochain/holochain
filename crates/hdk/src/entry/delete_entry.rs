/// Alias to delete!
///
/// Takes any expression that evaluates to the HeaderHash of the deleted element.
///
/// ```ignore
/// delete_entry!(entry_hash!(foo_entry)?)?;
/// ```
#[macro_export]
macro_rules! delete_entry {
    ( $hash:expr ) => {{
        delete!($hash)
    }};
}
