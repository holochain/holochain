#[macro_export]
macro_rules! entry_def {
    ( $t:ident $def:expr ) => {
        impl $t {
            pub fn entry_def() -> $crate::prelude::EntryDef {
                $def
            }

            pub fn entry_def_id() -> $crate::prelude::EntryDefId {
                Self::entry_def().id
            }

            pub fn entry_visibility() -> $crate::prelude::EntryVisibility {
                Self::entry_def().visibility
            }

            pub fn crdt_type() -> $crate::prelude::CrdtType {
                Self::entry_def().crdt_type
            }

            pub fn required_validations() -> $crate::prelude::RequiredValidations {
                Self::entry_def().required_validations
            }
        }

        impl TryFrom<&$crate::prelude::Entry> for $t {
            type Error = $crate::prelude::SerializedBytesError;
            fn try_from(entry: &$crate::prelude::Entry) -> Result<Self, Self::Error> {
                match entry {
                    Entry::App(sb) => Ok(Self::try_from(sb.to_owned())?),
                    _ => Err($crate::prelude::SerializedBytesError::FromBytes(format!(
                        "{:?} is not an Entry::App so has no serialized bytes",
                        entry
                    ))),
                }
            }
        }

        impl TryFrom<$crate::prelude::Entry> for $t {
            type Error = $crate::prelude::SerializedBytesError;
            fn try_from(entry: $crate::prelude::Entry) -> Result<Self, Self::Error> {
                Self::try_from(&entry)
            }
        }

        impl From<$t> for $crate::prelude::EntryDef {
            fn from(_: $t) -> Self {
                $t::entry_def()
            }
        }

        impl From<&$t> for $crate::prelude::EntryDef {
            fn from(_: &$t) -> Self {
                $t::entry_def()
            }
        }

        impl From<$t> for $crate::prelude::EntryDefId {
            fn from(_: $t) -> Self {
                $t::entry_def_id()
            }
        }

        impl From<&$t> for $crate::prelude::EntryDefId {
            fn from(_: &$t) -> Self {
                $t::entry_def_id()
            }
        }

        impl From<$t> for $crate::prelude::EntryVisibility {
            fn from(_: $t) -> Self {
                $t::entry_visibility()
            }
        }

        impl From<&$t> for $crate::prelude::EntryVisibility {
            fn from(_: &$t) -> Self {
                $t::entry_visibility()
            }
        }

        impl From<$t> for $crate::prelude::CrdtType {
            fn from(_: $t) -> Self {
                $t::crdt_type()
            }
        }

        impl From<&$t> for $crate::prelude::CrdtType {
            fn from(_: &$t) -> Self {
                $t::crdt_type()
            }
        }

        impl From<$t> for $crate::prelude::RequiredValidations {
            fn from(_: $t) -> Self {
                $t::required_validations()
            }
        }

        impl From<&$t> for $crate::prelude::RequiredValidations {
            fn from(_: &$t) -> Self {
                $t::required_validations()
            }
        }
    };
}

#[macro_export]
macro_rules! entry_defs {
    // @todo make this work for more than one def
    ( def $t:ident $def:expr; ) => {
        $crate::entry_def!($t $def);
        $crate::entry_defs!(vec![
            $t::entry_def()
        ]);
    };
    ( $defs_vec:expr ) => {
        fn __entry_defs(_: ()) -> Result<$crate::prelude::EntryDefsCallbackResult, $crate::prelude::WasmError> {
            Ok($crate::prelude::EntryDefsCallbackResult::Defs($defs_vec.into()))
        }
        $crate::map_extern!(entry_defs, __entry_defs);
    };
}
