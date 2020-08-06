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
