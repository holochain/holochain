#[macro_export]
macro_rules! map_extern {
    ( $name:tt, $f:ident ) => {
        #[no_mangle]
        pub extern "C" fn $name(ptr: GuestPtr) -> GuestPtr {
            let input: HostInput = host_args!(ptr);
            let result = $f(try_result!(
                input.into_inner().try_into(),
                "failed to deserialize args"
            ));
            let result_value = try_result!(result, "inner function failed");
            let result_sb = try_result!(
                SerializedBytes::try_from(result_value),
                "inner function result serialization error"
            );
            ret!(GuestOutput::new(result_sb));
        }
    };
}

#[macro_export]
macro_rules! entry_defs {
    ( $defs_vec:expr ) => {
        fn __entry_defs(_: ()) -> Result<EntryDefsCallbackResult, WasmError> {
            Ok(EntryDefsCallbackResult::Defs($defs_vec.into()))
        }
        map_extern!(entry_defs, __entry_defs);
    };
}

#[macro_export]
macro_rules! api_call {
    ( $f:ident, $input:expr, $outputt:ty ) => {{
        holochain_wasmer_guest::holochain_externs!();

        let result: Result<$outputt, $crate::prelude::SerializedBytesError> =
            $crate::prelude::host_call!($f, $input);
        result.map(|r| r.into_inner())
    }};
}

#[macro_export]
macro_rules! zome_info {
    () => {{
        $crate::api_call!(__zome_info, ZomeInfoInput::new(()), ZomeInfoOutput)
    }};
}

#[macro_export]
macro_rules! agent_info {
    () => {{
        $crate::api_call!(__agent_info, AgentInfoInput::new(()), AgentInfoOutput)
    }};
}

#[macro_export]
macro_rules! call_remote {
    ( $agent:expr, $zome:expr, $fn_name:expr, $cap:expr, $request:expr ) => {{
        $crate::api_call!(
            __call_remote,
            CallRemoteInput::new(CallRemote::new($agent, $zome, $fn_name, $cap, $request)),
            CallRemoteOutput
        )
    }};
}

#[macro_export]
macro_rules! debug {
    ( $msg:expr ) => {{
        $crate::api_call!(
            __debug,
            DebugInput::new(debug_msg!(format!("{:?}", $msg))),
            DebugOutput
        )
    }};
}

#[macro_export]
macro_rules! commit_entry {
    ( $input:expr ) => {{
        let try_sb = SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::api_call!(
                __commit_entry,
                CommitEntryInput::new(($input.into(), Entry::App(sb))),
                CommitEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}

#[macro_export]
macro_rules! entry_hash {
    ( $input:expr ) => {{
        let try_sb = SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::api_call!(
                __entry_hash,
                EntryHashInput::new(Entry::App(sb)),
                EntryHashOutput
            ),
            Err(e) => Err(e),
        }
    }};
}

#[macro_export]
macro_rules! get_entry {
    ( $hash:expr, $options:expr ) => {{
        $crate::api_call!(
            __get_entry,
            GetEntryInput::new(($hash, $options)),
            GetEntryOutput
        )
    }};
    ( $input:expr ) => {
        get_entry!($input, $crate::prelude::GetOptions)
    };
}

#[macro_export]
macro_rules! link_entries {
    ( $base:expr, $target:expr ) => {
        link_entries!($base, $target, vec![])
    };
    ( $base:expr, $target:expr, $tag:expr ) => {{
        $crate::api_call!(
            __link_entries,
            LinkEntriesInput::new(($base, $target, $tag.into())),
            LinkEntriesOutput
        )
    }};
}

#[macro_export]
macro_rules! remove_link {
    ( $add_link_header:expr ) => {{
        $crate::api_call!(
            __remove_link,
            RemoveLinkInput::new($add_link_header),
            RemoveLinkOutput
        )
    }};
}

#[macro_export]
macro_rules! get_links {
    ( $base:expr ) => {
        get_links!($base, None)
    };
    ( $base:expr, $tag:expr ) => {{
        $crate::api_call!(
            __get_links,
            GetLinksInput::new(($base, $tag.into())),
            GetLinksOutput
        )
    }};
}
