use crate::prelude::*;

/// Create an app entry.
///
/// An app entry is anything that the app can define a type for that matches the entry defs and
/// that can be serialized to `SerializedBytes`.
///
/// Accepts any input that implements TryInto<HdkEntry>.
/// The default impls from the `#[hdk_entry( .. )]` and `entry_def!()` macros include this.
///
/// e.g.
/// ```ignore
/// #[hdk_entry(id = "foo")]
/// pub struct Foo(u32);
/// create_entry(Foo(50))?;
/// ```
///
/// @see get and get_details for more information on CRUD
pub fn create_entry<I, E>(input: I) -> HdkResult<HeaderHash>
where
    HdkEntry: TryFrom<I, Error = E>,
    HdkError: From<E>,
{
    let HdkEntry(entry_def_id, entry) = HdkEntry::try_from(input)?;
    create(entry_def_id, entry)
}
