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
            __entry_hash,
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
    ( $base:expr, $target:expr, $tag:expr ) => {{
        $crate::api_call!(
            __link_entries,
            LinkEntriesInput::new(($base, $target, $tag.into())),
            LinkEntriesOutput
        )
    }};
}

#[macro_export]
macro_rules! get_links {
    ( $base:expr, $tag:expr ) => {
        $crate::api_call!(
            __get_links,
            GetLinksInput::new(($base, $tag.into())),
            GetLinksOutput
        )
    };
}
