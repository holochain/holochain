/// Trait for binding static `EntryDef` property access for a type.
/// @see register_entry!
pub trait EntryDefRegistration {
    fn entry_def() -> crate::prelude::EntryDef;

    fn entry_def_id() -> crate::prelude::EntryDefId;

    fn entry_visibility() -> crate::prelude::EntryVisibility;

    fn crdt_type() -> crate::prelude::CrdtType;

    fn required_validations() -> crate::prelude::RequiredValidations;
}

/// Implements conversion traits to allow a struct to be handled as an app entry.
/// If you have some need to implement custom serialization logic or metadata injection
/// you can do so by implementing these traits manually instead.
///
/// This requires that TryFrom and TryInto SerializedBytes is implemented for the entry type,
/// which implies that serde::Serialize and serde::Deserialize is also implemented.
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
                    _ => Err($crate::prelude::SerializedBytesError::Deserialize(format!(
                        "{:?} is not an Entry::App so has no serialized bytes",
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

        impl TryFrom<&$t> for $crate::prelude::Entry {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: &$t) -> Result<Self, Self::Error> {
                match AppEntryBytes::try_from(SerializedBytes::try_from(t)?) {
                    Ok(app_entry_bytes) => Ok(Self::App(app_entry_bytes)),
                    Err(entry_error) => match entry_error {
                        EntryError::SerializedBytes(serialized_bytes_error) => {
                            Err(WasmError::Serialize(serialized_bytes_error))
                        }
                        EntryError::EntryTooLarge(_) => {
                            Err(WasmError::Guest(entry_error.to_string()))
                        }
                    },
                }
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
/// For most normal applications, you should use the `entry_def!` macro instead.
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

            fn crdt_type() -> $crate::prelude::CrdtType {
                Self::entry_def().crdt_type
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

        impl TryFrom<&$t> for $crate::prelude::EntryWithDefId
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: &$t) -> Result<Self, Self::Error> {
                Ok(Self::new($t::entry_def_id(), t.try_into()?))
            }
        }

        impl TryFrom<$t> for $crate::prelude::EntryWithDefId {
            type Error = $crate::prelude::WasmError;
            fn try_from(t: $t) -> Result<Self, Self::Error> {
                (&t).try_into()
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

        impl From<$t> for $crate::prelude::CrdtType
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: $t) -> Self {
                $t::crdt_type()
            }
        }

        impl From<&$t> for $crate::prelude::CrdtType
        where
            $t: $crate::prelude::EntryDefRegistration,
        {
            fn from(_: &$t) -> Self {
                $t::crdt_type()
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
/// If you don't want to use the macro you can simply implement similar fns youself.
///
/// This is not a trait at the moment, it could be in the future but for now these functions and
/// impls are just a loose set of conventions.
///
/// It's actually entirely possible to interact with core directly without any of these.
/// e.g. create_entry() is just building a tuple of EntryDefId and Entry::App under the hood.
///
/// This requires that TryFrom and TryInto SerializedBytes is implemented for the entry type,
/// which implies that serde::Serialize and serde::Deserialize is also implemented.
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
        fn entry_defs(_: ()) -> $crate::prelude::ExternResult<$crate::prelude::EntryDefsCallbackResult> {
            Ok($crate::prelude::EntryDefsCallbackResult::from(vec![ $( $def ),* ]))
        }
    };
}

/// Attempts to lookup the EntryDefIndex given an EntryDefId.
///
/// The EntryDefId is a String newtype and the EntryDefIndex is a u8 newtype.
/// The EntryDefIndex is used to reference the entry type in headers on the DHT and as the index of the type exported to tooling.
/// The EntryDefId is the 'human friendly' string that the entry_defs callback maps to the index.
///
/// The host actually has no idea how to do this mapping, it is provided by the wasm!
///
/// Therefore this is a macro that calls the `entry_defs` callback as defined within a zome directly from the zome.
/// It is a macro so that we can call a function with a known name `crate::entry_defs` from the HDK before the function is defined.
///
/// Obviously this assumes and requires that a compliant `entry_defs` callback _is_ defined at the root of the crate.
#[macro_export]
macro_rules! entry_def_index {
    ( $t:ty ) => {
        match crate::entry_defs(()) {
            Ok($crate::prelude::EntryDefsCallbackResult::Defs(entry_defs)) => {
                match entry_defs.entry_def_index_from_id(<$t>::entry_def_id()) {
                    Some(entry_def_index) => Ok::<
                        $crate::prelude::EntryDefIndex,
                        $crate::prelude::WasmError,
                    >(entry_def_index),
                    None => {
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
            Ok($crate::prelude::EntryDefsCallbackResult::Err(error)) => {
                $crate::prelude::tracing::error!(?error, "Failed to lookup entry defs.");
                Err::<$crate::prelude::EntryDefIndex, $crate::prelude::WasmError>(
                    $crate::prelude::WasmError::Guest(error),
                )
            }
            Err(error) => {
                $crate::prelude::tracing::error!(?error, "Failed to lookup entry defs.");
                Err::<$crate::prelude::EntryDefIndex, $crate::prelude::WasmError>(error)
            }
        }
    };
}
