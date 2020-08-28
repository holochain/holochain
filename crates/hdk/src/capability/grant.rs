#[macro_export]
macro_rules! commit_cap_grant {
    ( $input:expr ) => {{
        $crate::host_fn!(
            __commit_entry,
            $crate::prelude::CommitEntryInput::new((
                $crate::prelude::EntryDefId::CapGrant,
                $crate::prelude::Entry::CapGrant($input)
            )),
            $crate::prelude::CommitEntryOutput
        )
    }};
}
