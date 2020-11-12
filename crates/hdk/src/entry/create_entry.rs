use crate::prelude::*;

/// Create an app entry.
///
/// An app entry is anything that the app can define a type for that matches the entry defs and
/// that can be serialized to `SerializedBytes`.
///
/// Accepts any expression that evaluates to something that implements TryInto<SerializedBytes> and
/// Into<EntryDefId>, so the defaults from the `#[hdk_entry( .. )]` and `entry_def!()` macros
/// make any struct/enum into an app entry.
///
/// e.g.
/// ```ignore
/// #[hdk_entry(id = "foo")]
/// pub struct Foo(u32);
/// create_entry(Foo(50))?;
/// ```
///
/// @see get! and get_details! for more information on CRUD
pub fn create_entry<'a, I: 'a>(input: &'a I) -> HdkResult<HeaderHash>
where
    EntryDefId: From<&'a I>,
    SerializedBytes: TryFrom<&'a I, Error = SerializedBytesError>,
{
    host_externs!(__create);
    let entry_def_id = EntryDefId::from(input);
    let sb = SerializedBytes::try_from(input)?;
    create!(entry_def_id, Entry::App(sb.try_into()?))
}
