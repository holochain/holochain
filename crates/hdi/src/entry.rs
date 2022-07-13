use crate::prelude::*;

pub use hdk_derive::hdk_entry_defs;
pub use hdk_derive::hdk_entry_helper;

#[cfg(doc)]
pub mod examples;

/// MUST get an EntryHashed at a given EntryHash.
///
/// The EntryHashed is NOT guaranteed to be associated with a valid (or even validated) Action/Record.
/// For example, an invalid Record could be published and `must_get_entry` would return the EntryHashed.
///
/// This may be useful during validation callbacks where the validity and relevance of some content can be
/// asserted by the CURRENT validation callback independent of a Record. This behaviour avoids the potential for
/// eclipse attacks to lie about the validity of some data and cause problems for a hApp.
/// If you NEED to know that a dependency is valid in order for the current validation logic
/// (e.g. inductive validation of a tree) then `must_get_valid_record` is likely what you need.
///
/// `must_get_entry` is available in contexts such as validation where both determinism and network access is desirable.
///
/// An EntryHashed will NOT be returned if:
/// - @TODO It is PURGED (community redacted entry)
/// - @TODO ALL actions pointing to it are WITHDRAWN by the authors
/// - ALL actions pointing to it are ABANDONED by ALL authorities due to validation failure
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

/// MUST get a SignedActionHashed at a given ActionHash.
///
/// The SignedActionHashed is NOT guaranteed to be a valid (or even validated) Record.
/// For example, an invalid Action could be published and `must_get_action` would return the `SignedActionHashed`.
///
/// This may be useful during validation callbacks where the validity depends on an action existing regardless of its associated Entry.
/// For example, we may simply need to check that the author is the same for two referenced Actions.
///
/// `must_get_action` is available in contexts such as validation where both determinism and network access is desirable.
///
/// A `SignedActionHashed` will NOT be returned if:
///
/// - @TODO The action is WITHDRAWN by the author
/// - @TODO The action is ABANDONED by ALL authorities
/// - Nobody knows about it on the currently visible network
///
/// If a `SignedActionHashed` fails to be returned:
///
/// - Callbacks will return early with `UnresolvedDependencies`
/// - Zome calls will receive a `WasmError` from the host
pub fn must_get_action(action_hash: ActionHash) -> ExternResult<SignedActionHashed> {
    HDI.with(|h| {
        h.borrow()
            .must_get_action(MustGetActionInput::new(action_hash))
    })
}

/// MUST get a VALID Record at a given ActionHash.
///
/// The Record is guaranteed to be valid.
/// More accurately the Record is guarantee to be consistently reported as valid by the visible network.
///
/// The validity requirement makes this more complex but notably enables inductive validation of arbitrary graph structures.
/// For example "If this Record is valid, and its parent is valid, up to the root, then the whole tree of Records is valid".
///
/// If at least one authority (1 of N trust) claims the Record is invalid then a conflict resolution/warranting round will be triggered.
///
/// In the case of a total eclipse (every visible authority is lying) then we cannot immediately detect an invalid Record.
/// Unlike `must_get_entry` and `must_get_action` we cannot simply inspect the cryptographic integrity to know this.
///
/// In theory we can run validation of the returned Record ourselves, which itself may be based on `must_get_X` calls.
/// If there is a large nested graph of `must_get_valid_record` calls this could be extremely heavy.
/// Note though that each "hop" in recursive validation is routed to a completely different set of authorities.
/// It does not take many hops to reach the point where an attacker needs to eclipse the entire network to lie about Record validity.
///
/// @TODO We keep signed receipts from authorities serving up "valid records".
/// - If we ever discover a record we were told is valid is invalid we can retroactively look to warrant authorities
/// - We can async (e.g. in a background task) be recursively validating Record dependencies ourselves, following hops until there is no room for lies
/// - We can with small probability recursively validate to several hops inline to discourage potential eclipse attacks with a credible immediate threat
///
/// If you do not care about validity and simply want a pair of Action+Entry data, then use both `must_get_action` and `must_get_entry` together.
///
/// `must_get_valid_record` is available in contexts such as validation where both determinism and network access is desirable.
///
/// An `Record` will not be returned if:
///
/// - @TODO It is WITHDRAWN by the author
/// - @TODO The Entry is PURGED by the community
/// - It is ABANDONED by ALL authorities due to failed validation
/// - If ANY authority (1 of N trust) OR ourselves (0 of N trust) believes it INVALID
/// - Nobody knows about it on the visible network
///
/// If an `Record` fails to be returned:
///
/// - Callbacks will return early with `UnresolvedDependencies`
/// - Zome calls will receive a `WasmError` from the host
pub fn must_get_valid_record(action_hash: ActionHash) -> ExternResult<Record> {
    HDI.with(|h| {
        h.borrow()
            .must_get_valid_record(MustGetValidRecordInput::new(action_hash))
    })
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
                    ).map_err(|e| $crate::prelude::wasm_error!(e.into()))?),
                    $crate::prelude::Entry::CounterSign(_, eb) => Ok(Self::try_from(
                        $crate::prelude::SerializedBytes::from(eb.to_owned()),
                    ).map_err(|e| $crate::prelude::wasm_error!(e.into()))?),
                    _ => Err($crate::prelude::wasm_error!($crate::prelude::SerializedBytesError::Deserialize(format!(
                        "{:?} is not an Entry::App or Entry::CounterSign so has no serialized bytes",
                        entry
                    ))
                    .into())),
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

        impl TryFrom<&$crate::prelude::Record> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(record: &$crate::prelude::Record) -> Result<Self, Self::Error> {
                Ok(match &record.entry {
                    RecordEntry::Present(entry) => Self::try_from(entry)?,
                    _ => return Err(
                        $crate::prelude::wasm_error!(
                        $crate::prelude::WasmErrorInner::Guest(format!("Tried to deserialize a record, expecting it to contain entry data, but there was none. Record ActionHash: {}", record.signed_action.hashed.hash))),
                    )
                })
            }
        }

        impl TryFrom<$crate::prelude::Record> for $t {
            type Error = $crate::prelude::WasmError;
            fn try_from(record: $crate::prelude::Record) -> Result<Self, Self::Error> {
                (&record).try_into()
            }
        }

        impl TryFrom<&$t> for $crate::prelude::AppEntryBytes {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: &$t) -> Result<Self, Self::Error> {
                AppEntryBytes::try_from(SerializedBytes::try_from(t).map_err(|e| wasm_error!(e.into()))?).map_err(|entry_error| match entry_error {
                    EntryError::SerializedBytes(serialized_bytes_error) => {
                        wasm_error!(WasmErrorInner::Serialize(serialized_bytes_error))
                    }
                    EntryError::EntryTooLarge(_) => {
                        wasm_error!(WasmErrorInner::Guest(entry_error.to_string()))
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
#[doc(hidden)]
#[macro_export]
macro_rules! entry_defs {
    [ $( $def:expr ),* ] => {
        #[hdk_extern]
        pub fn entry_defs(_: ()) -> $crate::prelude::ExternResult<$crate::prelude::EntryDefsCallbackResult> {
            Ok($crate::prelude::EntryDefsCallbackResult::from(vec![ $( $def ),* ]))
        }
    };
}
