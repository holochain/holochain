use crate::prelude::*;

/// Create an app entry.
///
/// An app entry is anything that the app can define a type for that matches the entry defs and
/// that can be serialized to `SerializedBytes`.
///
/// Accepts any input that implements TryInto<EntryWithDefId>.
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
    EntryWithDefId: TryFrom<I, Error = E>,
    HdkError: From<E>,
{
    create(EntryWithDefId::try_from(input)?)
}
