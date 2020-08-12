//! # HDK API
//!
//! Welcome to the HDK 3.0.
//! This HDK is currently in flux, expect rapid changes.
//! Currently there are helper macros to aid in writing happs.
//!
//! # Examples
//!
//! ## map_extern!
//!
//! ```
//! # #[macro_use] extern crate hdk3;
//! # fn main() {
//! use hdk3::prelude::*;
//!
//! #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
//! pub struct MyInput;
//!
//! #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
//! pub struct MyOutput(MyInput);
//!
//! fn _foo(input: MyInput) -> Result<MyOutput, WasmError> {
//!   Ok(MyOutput(input))
//! }
//!
//! map_extern!(foo, _foo);
//! # }
//! ```
//!
//! ## entry_def! & entry_defs
//!
//! ```
//! # #[macro_use] extern crate hdk3;
//! # fn main() {
//! # use hdk3::prelude::*;
//! #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
//! pub struct Foo;
//!
//! #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
//! pub struct Bar;
//!
//! const FOO_ID: &str = "foo";
//! const BAR_ID: &str = "bar";
//!
//! // Long version
//! entry_def!(Foo EntryDef {
//!     id: FOO_ID.into(),
//!     crdt_type: CrdtType,
//!     required_validations: RequiredValidations::default(),
//!     visibility: EntryVisibility::Public,
//! });
//!
//! // Short version
//! entry_def!(Bar EntryDef {
//!     id: BAR_ID.into(),
//!     ..Default::default()
//! });
//!
//! entry_defs!(vec![Foo::entry_def(), Bar::entry_def()]);
//! # }
//! ```
//!
//! ## commit_entry!, get!, entry_hash!, link_entries!, get_links!, debug!
//!
//! ```no_run
//! # fn main() -> Result<(), hdk3::prelude::WasmError> {
//! # use hdk3::prelude::*;
//! # #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
//! # pub struct Foo;
//! # #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
//! # pub struct Bar;
//! # const FOO_ID: &str = "foo";
//! # const BAR_ID: &str = "bar";
//! # entry_def!(Foo EntryDef {
//! #     id: FOO_ID.into(),
//! #     ..Default::default()
//! # });
//! # entry_def!(Bar EntryDef {
//! #     id: BAR_ID.into(),
//! #     ..Default::default()
//! # });
//! // Create your entry types
//! let foo = Foo;
//! let bar = Bar;
//! // Commit the entries
//! let _foo_header_hash = commit_entry!(foo.clone())?;
//! let _bar_header_hash = commit_entry!(bar.clone())?;
//! // Get the entry hash of each entry
//! let foo_entry_hash = entry_hash!(foo)?;
//! let bar_entry_hash = entry_hash!(bar)?;
//! // Link from foo (base) to bar (target)
//! let _link_add_header_hash = link_entries!(foo_entry_hash.clone(), bar_entry_hash)?;
//! // Get the links back
//! let links = get_links!(foo_entry_hash)?;
//! // Print out the links
//! debug!(links)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## call_remote!, zome_info!, agent_info!
//!
//! ```no_run
//! # fn main() -> Result<(), hdk3::prelude::WasmError> {
//! # use hdk3::prelude::*;
//! # #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
//! # pub struct MyInput;
//! # #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
//! # pub struct MyOutput(MyInput);
//! # let my_friends_agent_pubkey = holo_hash::AgentPubKey::from_raw_bytes(vec![b'0']);
//! // Get your agent key
//! let agent_pubkey = agent_info!()?.agent_pubkey;
//! // Get the name of this zome
//! let zome_name = zome_info!()?.zome_name;
//! // Call your friends foo function
//! let result: SerializedBytes = call_remote!(
//!     my_friends_agent_pubkey,
//!     zome_name,
//!     "foo".to_string(),
//!     CapSecret::default(),
//!     MyInput.try_into()?
//! )?;
//! // Get their output
//! let output: MyOutput = result.try_into()?;
//! // Print their output
//! debug!(output)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Direct Api Call
//! The above macros are convenience macros for calling the api but this
//! can also be done directly as follows:
//!
//! ```no_run
//! # fn main() -> Result<(), hdk3::prelude::WasmError> {
//! # use hdk3::prelude::*;
//! # #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
//! # pub struct Foo;
//! # #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
//! # pub struct Bar;
//! # const FOO_ID: &str = "foo";
//! # const BAR_ID: &str = "bar";
//! # entry_def!(Foo EntryDef {
//! #     id: FOO_ID.into(),
//! #     ..Default::default()
//! # });
//! # entry_def!(Bar EntryDef {
//! #     id: BAR_ID.into(),
//! #     ..Default::default()
//! # });
//! # // Create your entry types
//! # let foo = Foo;
//! # let bar = Bar;
//! // Commit foo
//! let foo_header_hash = commit_entry!(foo.clone())?;
//! // Call the api directly:
//! // Create the Entry from bar.
//! let entry = Entry::App(bar.clone().try_into()?);
//! // Call the update_entry host_fn directly
//! let _bar_header_hash = hdk3::api_call!(
//!     __update_entry,
//!     UpdateEntryInput::new((bar.clone().into(), entry, foo_header_hash)),
//!     UpdateEntryOutput
//! )?;
//! # Ok(())
//! # }
//! ```

#[macro_export]
macro_rules! map_extern {
    ( $name:tt, $f:ident ) => {
        #[no_mangle]
        pub extern "C" fn $name(ptr: $crate::prelude::GuestPtr) -> $crate::prelude::GuestPtr {
            let input: $crate::prelude::HostInput = $crate::prelude::host_args!(ptr);
            let result = $f($crate::prelude::try_result!(
                input.into_inner().try_into(),
                "failed to deserialize args"
            ));
            let result_value = $crate::prelude::try_result!(result, "inner function failed");
            let result_sb = $crate::prelude::try_result!(
                $crate::prelude::SerializedBytes::try_from(result_value),
                "inner function result serialization error"
            );
            $crate::prelude::ret!($crate::prelude::GuestOutput::new(result_sb));
        }
    };
}

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

#[macro_export]
macro_rules! api_call {
    ( $f:ident, $input:expr, $outputt:ty ) => {{
        $crate::prelude::holochain_externs!();

        let result: Result<$outputt, $crate::prelude::SerializedBytesError> =
            $crate::prelude::host_call!($f, $input);
        result.map(|r| r.into_inner())
    }};
}

#[macro_export]
macro_rules! zome_info {
    () => {{
        $crate::api_call!(
            __zome_info,
            $crate::prelude::ZomeInfoInput::new(()),
            $crate::prelude::ZomeInfoOutput
        )
    }};
}

#[macro_export]
macro_rules! agent_info {
    () => {{
        $crate::api_call!(
            __agent_info,
            $crate::prelude::AgentInfoInput::new(()),
            $crate::prelude::AgentInfoOutput
        )
    }};
}

#[macro_export]
macro_rules! call_remote {
    ( $agent:expr, $zome:expr, $fn_name:expr, $cap:expr, $request:expr ) => {{
        $crate::api_call!(
            __call_remote,
            $crate::prelude::CallRemoteInput::new($crate::prelude::CallRemote::new(
                $agent, $zome, $fn_name, $cap, $request
            )),
            $crate::prelude::CallRemoteOutput
        )
    }};
}

#[macro_export]
macro_rules! debug {
    ( $msg:expr ) => {{
        $crate::api_call!(
            __debug,
            $crate::prelude::DebugInput::new($crate::prelude::debug_msg!(format!("{:?}", $msg))),
            $crate::prelude::DebugOutput
        )
    }};
}

#[macro_export]
macro_rules! commit_entry {
    ( $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::api_call!(
                __commit_entry,
                $crate::prelude::CommitEntryInput::new((
                    $input.into(),
                    $crate::prelude::Entry::App(sb)
                )),
                $crate::prelude::CommitEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}

#[macro_export]
macro_rules! entry_hash {
    ( $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::api_call!(
                __entry_hash,
                $crate::prelude::EntryHashInput::new($crate::prelude::Entry::App(sb)),
                $crate::prelude::EntryHashOutput
            ),
            Err(e) => Err(e),
        }
    }};
}

#[macro_export]
macro_rules! get {
    ( $hash:expr, $options:expr ) => {{
        $crate::api_call!(
            __get,
            $crate::prelude::GetInput::new(($hash.into(), $options)),
            $crate::prelude::GetOutput
        )
    }};
    ( $input:expr ) => {
        get!($input, $crate::prelude::GetOptions)
    };
}

#[macro_export]
macro_rules! link_entries {
    ( $base:expr, $target:expr ) => {
        $crate::link_entries!($base, $target, vec![])
    };
    ( $base:expr, $target:expr, $tag:expr ) => {{
        $crate::api_call!(
            __link_entries,
            $crate::prelude::LinkEntriesInput::new(($base, $target, $tag.into())),
            $crate::prelude::LinkEntriesOutput
        )
    }};
}

#[macro_export]
macro_rules! remove_link {
    ( $add_link_header:expr ) => {{
        $crate::api_call!(
            __remove_link,
            $crate::prelude::RemoveLinkInput::new($add_link_header),
            $crate::prelude::RemoveLinkOutput
        )
    }};
}

#[macro_export]
macro_rules! get_links {
    ( $base:expr ) => {
        $crate::get_links!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::api_call!(
            __get_links,
            $crate::prelude::GetLinksInput::new(($base, $tag.into())),
            $crate::prelude::GetLinksOutput
        )
    }};
}

#[macro_export]
macro_rules! get_link_details {
    ( $base:expr ) => {
        $crate::get_link_details!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::api_call!(
            __get_link_details,
            GetLinkDetailsInput::new(($base, $tag.into())),
            GetLinkDetailsOutput
        )
    }};
}
