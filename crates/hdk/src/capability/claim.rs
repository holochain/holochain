#[macro_export]
macro_rules! commit_cap_claim {
    ( $input:expr ) => {{
        $crate::host_fn!(
            __commit_entry,
            $crate::prelude::CommitEntryInput::new((
                $crate::prelude::EntryDefId::CapClaim,
                $crate::prelude::Entry::CapClaim($input)
            )),
            $crate::prelude::CommitEntryOutput
        )
    }};
}
