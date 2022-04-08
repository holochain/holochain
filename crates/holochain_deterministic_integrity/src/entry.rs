use crate::prelude::*;

/// MUST get an EntryHashed at a given EntryHash.
///
/// The EntryHashed is NOT guaranteed to be associated with a valid (or even validated) Header/Element.
/// For example, an invalid Element could be published and `must_get_entry` would return the EntryHashed.
///
/// This may be useful during validation callbacks where the validity and relevance of some content can be
/// asserted by the CURRENT validation callback independent of an Element. This behaviour avoids the potential for
/// eclipse attacks to lie about the validity of some data and cause problems for a hApp.
/// If you NEED to know that a dependency is valid in order for the current validation logic
/// (e.g. inductive validation of a tree) then `must_get_valid_element` is likely what you need.
///
/// `must_get_entry` is available in contexts such as validation where both determinism and network access is desirable.
///
/// An EntryHashed will NOT be returned if:
/// - @TODO It is PURGED (community redacted entry)
/// - @TODO ALL headers pointing to it are WITHDRAWN by the authors
/// - ALL headers pointing to it are ABANDONED by ALL authorities due to validation failure
/// - Nobody knows about it on the currently visible network
///
/// If an EntryHashed fails to be returned:
///
/// - Callbacks will return early with `UnresolvedDependencies`
/// - Zome calls will receive a `WasmError` from the host
pub fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    HDI.with(|h| {
        h.borrow()
            .must_get_entry(MustGetEntryInput::new(entry_hash))
    })
}

/// MUST get a SignedHeaderHashed at a given HeaderHash.
///
/// The SignedHeaderHashed is NOT guaranteed to be a valid (or even validated) Element.
/// For example, an invalid Header could be published and `must_get_header` would return the `SignedHeaderHashed`.
///
/// This may be useful during validation callbacks where the validity depends on a Header existing regardless of its associated Entry.
/// For example, we may simply need to check that the author is the same for two referenced Headers.
///
/// `must_get_header` is available in contexts such as validation where both determinism and network access is desirable.
///
/// A `SignedHeaderHashed` will NOT be returned if:
///
/// - @TODO The header is WITHDRAWN by the author
/// - @TODO The header is ABANDONED by ALL authorities
/// - Nobody knows about it on the currently visible network
///
/// If a `SignedHeaderHashed` fails to be returned:
///
/// - Callbacks will return early with `UnresolvedDependencies`
/// - Zome calls will receive a `WasmError` from the host
pub fn must_get_header(header_hash: HeaderHash) -> ExternResult<SignedHeaderHashed> {
    HDI.with(|h| {
        h.borrow()
            .must_get_header(MustGetHeaderInput::new(header_hash))
    })
}

/// MUST get a VALID Element at a given HeaderHash.
///
/// The Element is guaranteed to be valid.
/// More accurately the Element is guarantee to be consistently reported as valid by the visible network.
///
/// The validity requirement makes this more complex but notably enables inductive validation of arbitrary graph structures.
/// For example "If this Element is valid, and its parent is valid, up to the root, then the whole tree of Elements is valid".
///
/// If at least one authority (1 of N trust) claims the Element is invalid then a conflict resolution/warranting round will be triggered.
///
/// In the case of a total eclipse (every visible authority is lying) then we cannot immediately detect an invalid Element.
/// Unlike `must_get_entry` and `must_get_header` we cannot simply inspect the cryptographic integrity to know this.
///
/// In theory we can run validation of the returned Element ourselves, which itself may be based on `must_get_X` calls.
/// If there is a large nested graph of `must_get_valid_element` calls this could be extremely heavy.
/// Note though that each "hop" in recursive validation is routed to a completely different set of authorities.
/// It does not take many hops to reach the point where an attacker needs to eclipse the entire network to lie about Element validity.
///
/// @TODO We keep signed receipts from authorities serving up "valid elements".
/// - If we ever discover an element we were told is valid is invalid we can retroactively look to warrant authorities
/// - We can async (e.g. in a background task) be recursively validating Element dependencies ourselves, following hops until there is no room for lies
/// - We can with small probability recursively validate to several hops inline to discourage potential eclipse attacks with a credible immediate threat
///
/// If you do not care about validity and simply want a pair of Header+Entry data, then use both `must_get_header` and `must_get_entry` together.
///
/// `must_get_valid_element` is available in contexts such as validation where both determinism and network access is desirable.
///
/// An `Element` will not be returned if:
///
/// - @TODO It is WITHDRAWN by the author
/// - @TODO The Entry is PURGED by the community
/// - It is ABANDONED by ALL authorities due to failed validation
/// - If ANY authority (1 of N trust) OR ourselves (0 of N trust) believes it INVALID
/// - Nobody knows about it on the visible network
///
/// If an `Element` fails to be returned:
///
/// - Callbacks will return early with `UnresolvedDependencies`
/// - Zome calls will receive a `WasmError` from the host
pub fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    HDI.with(|h| {
        h.borrow()
            .must_get_valid_element(MustGetValidElementInput::new(header_hash))
    })
}

/// Trait for binding static [`EntryDef`] property access for a type.
/// See [`register_entry`]
pub trait EntryDefRegistration {
    fn entry_def() -> crate::prelude::EntryDef;

    fn entry_def_id() -> crate::prelude::EntryDefId;

    fn entry_visibility() -> crate::prelude::EntryVisibility;

    fn required_validations() -> crate::prelude::RequiredValidations;
}

/// Implements conversion traits to allow a struct to be handled as an app entry.
/// If you have some need to implement custom serialization logic or metadata injection
/// you can do so by implementing these traits manually instead.
///
/// This requires that TryFrom and TryInto [`derive@SerializedBytes`] is implemented for the entry type,
/// which implies that [`serde::Serialize`] and [`serde::Deserialize`] is also implemented.
/// These can all be derived and there is an attribute macro that both does the default defines.
#[macro_export]
macro_rules! app_entry {
    ( $t:ident ) => {
        impl TryFrom<&$crate::prelude::Entry> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(entry: &$crate::prelude::Entry) -> Result<Self, Self::Error> {
                match entry {
                    $crate::prelude::Entry::App(eb) => Ok(Self::try_from(
                        $crate::prelude::SerializedBytes::from(eb.to_owned()),
                    )?),
                    $crate::prelude::Entry::CounterSign(_, eb) => Ok(Self::try_from(
                        $crate::prelude::SerializedBytes::from(eb.to_owned()),
                    )?),
                    _ => Err($crate::prelude::SerializedBytesError::Deserialize(format!(
                        "{:?} is not an Entry::App or Entry::CounterSign so has no serialized bytes",
                        entry
                    ))
                    .into()),
                }
            }
        }

        impl TryFrom<$crate::prelude::Entry> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(entry: $crate::prelude::Entry) -> Result<Self, Self::Error> {
                Self::try_from(&entry)
            }
        }

        impl TryFrom<$crate::prelude::EntryHashed> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(entry_hashed: $crate::prelude::EntryHashed) -> Result<Self, Self::Error> {
                Self::try_from(entry_hashed.as_content())
            }
        }

        impl TryFrom<&$crate::prelude::Element> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(element: &$crate::prelude::Element) -> Result<Self, Self::Error> {
                Ok(match &element.entry {
                    ElementEntry::Present(entry) => Self::try_from(entry)?,
                    _ => return Err(Self::Error::Guest(format!("Tried to deserialize an element, expecting it to contain entry data, but there was none. Element HeaderHash: {}", element.signed_header.hashed.hash))),
                })
            }
        }

        impl TryFrom<$crate::prelude::Element> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(element: $crate::prelude::Element) -> Result<Self, Self::Error> {
                (&element).try_into()
            }
        }

        impl TryFrom<&$t> for $crate::prelude::AppEntryBytes {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: &$t) -> Result<Self, Self::Error> {
                AppEntryBytes::try_from(SerializedBytes::try_from(t)?).map_err(|entry_error| match entry_error {
                    EntryError::SerializedBytes(serialized_bytes_error) => {
                        WasmError::Serialize(serialized_bytes_error)
                    }
                    EntryError::EntryTooLarge(_) => {
                        WasmError::Guest(entry_error.to_string())
                    }
                })
            }
        }

        impl TryFrom<$t> for $crate::prelude::AppEntryBytes {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: $t) -> Result<Self, Self::Error> {
                Self::try_from(&t)
            }
        }

        impl TryFrom<&$t> for $crate::prelude::Entry {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: &$t) -> Result<Self, Self::Error> {
                Ok(Self::App($crate::prelude::AppEntryBytes::try_from(t)?))
            }
        }

        impl TryFrom<$t> for $crate::prelude::Entry {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: $t) -> Result<Self, Self::Error> {
                Self::try_from(&t)
            }
        }
    };
}

/// Implements a whole lot of sane defaults for a struct or enum that should behave as an entry,
/// *without* implementing the app entry conversion interface.
///
/// This allows crates to easily define a struct as an entry separately to binding that struct
/// as an entry type in a dependent crate.
///
/// For most normal applications, you should use the [`entry_def!`] macro instead.
#[macro_export]
macro_rules! register_entry {
    ( $t:ident $def:expr ) => {
        impl $crate::prelude::EntryDefRegistration for $t {
            fn entry_def() -> $crate::prelude::EntryDef {
                $def
            }

            fn entry_def_id() -> $crate::prelude::EntryDefId {
                Self::entry_def().id
            }

            fn entry_visibility() -> $crate::prelude::EntryVisibility {
                Self::entry_def().visibility
            }

            fn required_validations() -> $crate::prelude::RequiredValidations {
                Self::entry_def().required_validations
            }
        }

        impl<'a> $crate::prelude::EntryDefRegistration for &'a $t {
            fn entry_def() -> $crate::prelude::EntryDef {
                $def
            }

            fn entry_def_id() -> $crate::prelude::EntryDefId {
                Self::entry_def().id
            }

            fn entry_visibility() -> $crate::prelude::EntryVisibility {
                Self::entry_def().visibility
            }

            fn required_validations() -> $crate::prelude::RequiredValidations {
                Self::entry_def().required_validations
            }
        }

        impl From<$t> for $crate::prelude::EntryDef
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: $t) -> Self {
                $t::entry_def()
            }
        }

        impl From<&$t> for $crate::prelude::EntryDef
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: &$t) -> Self {
                $t::entry_def()
            }
        }

        impl From<$t> for $crate::prelude::EntryDefId
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: $t) -> Self {
                $t::entry_def_id()
            }
        }

        impl From<&$t> for $crate::prelude::EntryDefId
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: &$t) -> Self {
                $t::entry_def_id()
            }
        }

        impl From<$t> for $crate::prelude::EntryVisibility
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: $t) -> Self {
                $t::entry_visibility()
            }
        }

        impl From<&$t> for $crate::prelude::EntryVisibility
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: &$t) -> Self {
                $t::entry_visibility()
            }
        }

        impl From<$t> for $crate::prelude::RequiredValidations
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: $t) -> Self {
                $t::required_validations()
            }
        }

        impl From<&$t> for $crate::prelude::RequiredValidations
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: &$t) -> Self {
                $t::required_validations()
            }
        }
    };
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
/// If you don't want to use the macro you can simply implement similar fns yourself.
///
/// This is not a trait at the moment, it could be in the future but for now these functions and
/// impls are just a loose set of conventions.
///
/// It's actually entirely possible to interact with core directly without any of these.
/// e.g. committing is just building a tuple of [`EntryDefId`] and [`Entry::App`] under the hood.
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
        app_entry!($t);
        register_entry!($t $def);
    };
}

/// Shorthand to implement the entry defs callback similar to the vec![ .. ] macro but for entries.
///
/// e.g. the following are the same
///
/// ```ignore
/// entry_defs![ Foo::entry_def() ];
/// ```
///
/// ```ignore
/// #[hdk_extern]
/// fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
///   Ok(vec![ Foo::entry_def() ].into())
/// }
/// ```
#[macro_export]
macro_rules! entry_defs {
    [ $( $def:expr ),* ] => {
        #[hdk_extern]
        pub fn entry_defs(_: ()) -> $crate::prelude::ExternResult<$crate::prelude::EntryDefsCallbackResult> {
            Ok($crate::prelude::EntryDefsCallbackResult::from(vec![ $( $def ),* ]))
        }
    };
}

/// Attempts to lookup the [`EntryDefIndex`] given an [`EntryDefId`].
///
/// The [`EntryDefId`] is a [`String`] newtype and the [`EntryDefIndex`] is a u8 newtype.
/// The [`EntryDefIndex`] is used to reference the entry type in headers on the DHT and as the index of the type exported to tooling.
/// The [`EntryDefId`] is the 'human friendly' string that the [`entry_defs!`] callback maps to the index.
///
/// The host actually has no idea how to do this mapping, it is provided by the wasm!
///
/// Therefore this is a macro that calls the [`entry_defs!`] callback as defined within a zome directly from the zome.
/// It is a macro so that we can call a function with a known name `crate::entry_defs` from the HDI before the function is defined.
///
/// Obviously this assumes and requires that a compliant [`entry_defs!`] callback _is_ defined at the root of the crate.
#[macro_export]
macro_rules! entry_def_index {
    ( $t:ty ) => {
        match $crate::prelude::zome_info() {
            Ok(ZomeInfo { entry_defs, .. }) => {
                match entry_defs.entry_def_index_from_id(<$t>::entry_def_id()) {
                    Some(entry_def_index) => Ok::<
                        $crate::prelude::EntryDefIndex,
                        $crate::prelude::WasmError,
                    >(entry_def_index),
                    None => {
                        #[cfg(feature = "trace")]
                        $crate::prelude::tracing::error!(
                            entry_def_type = stringify!($t),
                            ?entry_defs,
                            "Failed to lookup index for entry def id."
                        );
                        Err::<$crate::prelude::EntryDefIndex, $crate::prelude::WasmError>(
                            $crate::prelude::WasmError::Guest(
                                "Failed to lookup index for entry def id.".into(),
                            ),
                        )
                    }
                }
            }
            Err(error) => {
                #[cfg(feature = "trace")]
                $crate::prelude::tracing::error!(?error, "Failed to lookup entry defs.");
                Err::<$crate::prelude::EntryDefIndex, $crate::prelude::WasmError>(error)
            }
        }
    };
}

#[macro_export]
macro_rules! entry_type {
    ( $t:ty ) => {
        match $crate::prelude::entry_def_index!($t) {
            Ok(e_id) => match $crate::prelude::zome_info() {
                Ok(ZomeInfo { id, .. }) => Ok($crate::prelude::EntryType::App(
                    $crate::prelude::AppEntryType::new(e_id, id, <$t>::entry_visibility()),
                )),
                Err(e) => Err(e),
                _ => unreachable!(),
            },
            Err(e) => Err(e),
        }
    };
}
