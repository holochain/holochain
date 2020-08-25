/// commits an update header+entry referencing the header hash of an existing create or update
///
/// updates must point to a create or another update _header_ hash and provide the new entry value
/// the host will automatically match the entry hash internally given the header
///
/// ```ignore
/// #[hdk_entry(id = "foo")]
/// struct Foo(u32);
///
/// let foo_zero_header_hash: HeaderHash = commit_entry!(Foo(0))?;
/// let foo_ten_update_header_hash: HeaderHash = update_entry!(foo_zero_header_hash, Foo(10))?;
/// ```
///
/// @see get! and get_details! for more information on CRUD
#[macro_export]
macro_rules! update_entry {
    ( $hash:expr, $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __update_entry,
                $crate::prelude::UpdateEntryInput::new((
                    $input.into(),
                    $crate::prelude::Entry::App(sb),
                    $hash
                )),
                $crate::prelude::UpdateEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
