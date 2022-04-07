use crate::prelude::*;

pub use holochain_deterministic_integrity::entry::*;

/// General function that can create any entry type.
///
/// This is used under the hood by [`create_entry`], [`create_cap_grant`] and [`create_cap_claim`].
///
/// The host builds a [`Create`] header for the passed entry value and commits a new element to the
/// chain.
///
/// Usually you don't need to use this function directly; it is the most general way to create an
/// entry and standardizes the internals of higher level create functions.
pub fn create(create_input: CreateInput) -> ExternResult<HeaderHash> {
    HDK.with(|h| h.borrow().create(create_input))
}

/// General function that can update any entry type.
///
/// This is used under the hood by [`update_entry`], [`update_cap_grant`] and `update_cap_claim`.
///
/// @todo implement update_cap_claim
///
/// The host builds an [`Update`] header for the passed entry value and commits a new update to the
/// chain.
///
/// Usually you don't need to use this function directly; it is the most general way to update an
/// entry and standardizes the internals of higher level update functions.
pub fn update(hash: HeaderHash, create_input: CreateInput) -> ExternResult<HeaderHash> {
    HDK.with(|h| h.borrow().update(UpdateInput::new(hash, create_input)))
}

/// General function that can delete any entry type.
///
/// This is used under the hood by [`delete_entry`], [`delete_cap_grant`] and `delete_cap_claim`.
///
/// @todo implement delete_cap_claim
///
/// The host builds a [`Delete`] header for the passed entry and commits a new element to the chain.
///
/// Usually you don't need to use this function directly; it is the most general way to delete an
/// entry and standardizes the internals of higher level delete functions.
pub fn delete<I, E>(delete_input: I) -> ExternResult<HeaderHash>
where
    DeleteInput: TryFrom<I, Error = E>,
    WasmError: From<E>,
{
    HDK.with(|h| h.borrow().delete(DeleteInput::try_from(delete_input)?))
}

/// Create an app entry. Also see [`create`].
///
/// Apps define app entries by registering entry def ids with the `entry_defs` callback and serialize the
/// entry content when committing to the source chain.
///
/// This function accepts any input that implements [`TryInto<CreateInput>`].
/// The default impls from the `#[hdk_entry( .. )]` and [`entry_def!`] macros include this.
///
/// With generic type handling it may make sense to directly construct [`CreateInput`] and [`create`].
///
/// e.g.
/// ```ignore
/// #[hdk_entry(id = "foo")]
/// pub struct Foo(u32);
/// create_entry(Foo(50))?;
/// ```
///
/// See [`get`] and [`get_details`] for more information on CRUD.
pub fn create_entry<I, E>(input: I) -> ExternResult<HeaderHash>
where
    I: EntryDefRegistration,
    Entry: TryFrom<I, Error = E>,
    WasmError: From<E>,
{
    create(try_into_create_input(input)?)
}

/// Delete an app entry. Also see [`delete`].
///
/// This function accepts the [`HeaderHash`] of the element to delete and optionally an argument to
/// specify the [`ChainTopOrdering`]. Refer to [`DeleteInput`] for details.
///
/// ```ignore
/// #[hdk_entry(id = "foo")]
/// struct Foo(u32);
///
/// let header_hash = create_entry(Foo(50))?;
/// let delete_entry_header_hash = delete_entry(header_hash.clone())?;
/// ```
///
/// with a specific [`ChainTopOrdering`]:
/// ```ignore
/// delete_entry(DeleteInput::new(header_hash.clone(), ChainTopOrdering::Relaxed)?;
/// ```
pub fn delete_entry<I, E>(delete_input: I) -> ExternResult<HeaderHash>
where
    DeleteInput: TryFrom<I, Error = E>,
    WasmError: From<E>,
{
    delete(delete_input)
}

/// Update an app entry. Also see [`update`].
///
/// The hash is the [`HeaderHash`] of the deleted element, the input is a [`TryInto<CreateInput>`].
///
/// Updates can reference Elements which contain Entry data -- namely, Creates and other Updates -- but
/// not Deletes or system Elements.
///
/// As updates can reference elements on other agent's source chains across unpredictable network
/// topologies, they are treated as a tree structure.
///
/// Many updates can point to a single create/update and continue to accumulate as long as agents
/// author them against that element. It is up to happ developers to decide how to ensure the tree
/// branches are walked appropriately and that updates point to the correct element, whatever that
/// means for the happ.
///
/// ```ignore
/// #[hdk_entry(id = "foo")]
/// struct Foo(u32);
///
/// let foo_zero_header_hash: HeaderHash = commit_entry!(Foo(0))?;
/// let foo_ten_update_header_hash: HeaderHash = update_entry(foo_zero_header_hash, Foo(10))?;
/// ```
///
/// @todo in the future this will be true because we will have the concept of 'redirects':
/// Works as an app entry delete+create.
///
/// See [`create_entry`]
/// See [`update`]
/// See [`delete_entry`]
pub fn update_entry<I, E>(hash: HeaderHash, input: I) -> ExternResult<HeaderHash>
where
    I: EntryDefRegistration,
    Entry: TryFrom<I, Error = E>,
    WasmError: From<E>,
{
    update(hash, try_into_create_input(input)?)
}

/// Gets an element for a given entry or header hash.
///
/// The behaviour of get changes subtly per the _type of the passed hash_.
/// A header hash returns the element for that header, i.e. header+entry or header+None.
/// An entry hash returns the "oldest live" element, i.e. header+entry.
///
/// An element is no longer live once it is referenced by a valid delete element.
/// An update to an element does not change its liveness.
/// See [`get_details`] for more information about how CRUD elements reference each other.
///
/// Note: [`get`] __always triggers and blocks on a network call__.
///       @todo implement a 'get optimistic' that returns based on the current opinion of the world
///       and performs network calls in the background so they are available 'next time'.
///
/// Note: Deletes are considered in the liveness but Updates are not currently followed
///       automatically due to the need for the happ to disambiguate update logic.
///       @todo implement 'redirect' logic so that updates are followed by [`get`].
///
/// Note: Updates typically point to a different entry hash than what they are updating but not
///       always, e.g. consider changing `foo` to `bar` back to `foo`. The entry hashes in a crud
///       tree can be circular but the header hashes are never circular.
///       In this case, deleting the create for foo would make the second update pointing to foo
///       the "oldest live" element.
///
/// Note: "oldest live" only relates to disambiguating many creates and updates from many authors
///       pointing to a single entry, it is not the "current value" of an entry in a CRUD sense.
///       e.g. If "foo" is created then updated to "bar", a [`get`] on the hash of "foo" will return
///            "foo" as part of an element with the "oldest live" header.
///            To discover "bar" the agent needs to call `get_details` and decide how it wants to
///            collapse many potential creates, updates and deletes down into a single or filtered
///            set of updates, to "walk the tree".
///       e.g. Updates could include a proof of work and a tree would collapse to a simple
///            blockchain if the agent follows the "heaviest chain".
///       e.g. Updates could represent turns in a 2-player game and the update with the newest
///            timestamp countersigned by both players represents an opt-in chain of updates with
///            support for casual "undo" with player's consent.
///       e.g. Domain/user names could be claimed on a "first come, first serve" basis with only
///            creates and deletes allowed by validation rules, the "oldest live" element _does_
///            represent the element pointing at the first agent to claim a name, but it could also
///            be checked manually by the app with `get_details`.
///
/// Note: "oldest live" is only as good as the information available to the authorities the agent
///       contacts on their current network partition, there could always be an older live entry
///       on another partition, and of course the oldest live entry could be deleted and no longer
///       be live.
pub fn get<H>(hash: H, options: GetOptions) -> ExternResult<Option<Element>>
where
    AnyDhtHash: From<H>,
{
    Ok(HDK
        .with(|h| {
            h.borrow()
                .get(vec![GetInput::new(AnyDhtHash::from(hash), options)])
        })?
        .into_iter()
        .next()
        .unwrap())
}

/// Get an element and its details for the entry or header hash passed in.
/// Returns [`None`] if the entry/header does not exist.
/// The details returned are a contextual mix of elements and header hashes.
///
/// Note: The return details will be inferred by the hash type passed in, be careful to pass in the
///       correct hash type for the details you want.
///
/// Note: If a header hash is passed in the element returned is the specified element.
///       If an entry hash is passed in all the headers (so implicitly all the elements) are
///       returned for the entry that matches that hash.
///       See [`get`] for more information about what "oldest live" means.
///
/// The details returned include relevant creates, updates and deletes for the hash passed in.
///
/// Creates are initial header/entry combinations (elements) produced by commit_entry! and cannot
/// reference other headers.
/// Updates and deletes both reference a specific header+entry combination.
/// Updates must reference another create or update header+entry.
/// Deletes must reference a create or update header+entry (nothing can reference a delete).
///
/// Full elements are returned for direct references to the passed hash.
/// Header hashes are returned for references to references to the passed hash.
///
/// [`Details`] for a header hash return:
/// - the element for this header hash if it exists
/// - all update and delete _elements_ that reference that specified header
///
/// [`Details`] for an entry hash return:
/// - all creates, updates and delete _elements_ that reference that entry hash
/// - all update and delete _elements_ that reference the elements that reference the entry hash
///
/// Note: Entries are just values, so can be referenced by many CRUD headers by many authors.
///       e.g. the number 1 or string "foo" can be referenced by anyone publishing CRUD headers at
///       any time they need to represent 1 or "foo" for a create, update or delete.
///       If you need to disambiguate entry values, provide uniqueness in the entry value such as
///       a unique hash (e.g. current chain head), timestamp (careful about collisions!), or random
///       bytes/uuid (see random_bytes() and the uuid rust crate that supports uuids from bytes).
///
/// Note: There are multiple header types that exist and operate entirely outside of CRUD elements
///       so they cannot reference or be referenced by CRUD, so are immutable or have their own
///       mutation logic (e.g. link create/delete) and will not be included in [`get_details`] results
///       e.g. the DNA itself, links, migrations, etc.
///       However the element will still be returned by [`get_details`] if a header hash is passed,
///       these header-only elements will have [`None`] as the entry value.
pub fn get_details<H: Into<AnyDhtHash>>(
    hash: H,
    options: GetOptions,
) -> ExternResult<Option<Details>> {
    Ok(HDK
        .with(|h| {
            h.borrow()
                .get_details(vec![GetInput::new(hash.into(), options)])
        })?
        .into_iter()
        .next()
        .unwrap())
}

/// Implements a whole lot of sane defaults for a struct or enum that should behave as an entry.
/// All the entry def fields are available as dedicated methods on the type and matching From impls
/// are provided for each. This allows for both Foo::entry_def() and EntryDef::from(Foo::new())
/// style logic which are both useful in different scenarios.
///
/// For example, the Foo::entry_def() style works best in the entry_defs callback as it doesn't
/// require an instantiated Foo in order to get the definition.
/// On the other hand, EntryDef::from(Foo::new()) works better when e.g. using create_entry() as
/// an instance of Foo already exists and we need the entry def id back for creates and updates.
///
/// If you don't want to use the macro you can simply implement similar fns youself.
///
/// This is not a trait at the moment, it could be in the future but for now these functions and
/// impls are just a loose set of conventions.
///
/// It's actually entirely possible to interact with core directly without any of these.
/// e.g. [`create_entry`] is just building a tuple of [`EntryDefId`] and [`Entry::App`] under the hood.
///
/// This requires that TryFrom and TryInto [`derive@SerializedBytes`] is implemented for the entry type,
/// which implies that [`serde::Serialize`] and [`serde::Deserialize`] is also implemented.
/// These can all be derived and there is an attribute macro that both does the default defines.
///
///  e.g. the following are equivalent
///
/// ```ignore
/// #[hdk_entry(id = "foo", visibility = "private", required_validations = 6, )]
/// pub struct Foo;
/// ```
///
/// ```ignore
/// #[derive(SerializedBytes, serde::Serialize, serde::Deserialize)]
/// pub struct Foo;
/// entry_def!(Foo EntryDef {
///   id: "foo".into(),
///   visibility: EntryVisibility::Private,
///   ..Default::default()
/// });
/// ```
#[macro_export]
macro_rules! entry_def {
    ( $t:ident $def:expr ) => {
        $crate::prelude::holochain_deterministic_integrity::app_entry!($t);
        $crate::prelude::holochain_deterministic_integrity::register_entry!($t $def);
    };
}

/// Helper to convert a type to [`CreateInput`].
/// This is needed because [`TryInto`] / [`TryFrom`] cannot be
/// impl on a generic type.
fn try_into_create_input<T, E>(t: T) -> Result<CreateInput, WasmError>
where
    T: EntryDefRegistration,
    Entry: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    Ok(CreateInput::new(
        T::entry_def_id(),
        t.try_into()?,
        ChainTopOrdering::default(),
    ))
}
