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

#[macro_export]
macro_rules! update_cap_grant {
    ( $hash:expr, $input:expr ) => {{
        $crate::host_fn!(
            __update_entry,
            $crate::prelude::UpdateEntryInput::new((
                $crate::prelude::EntryDefId::CapGrant,
                $crate::prelude::Entry::CapGrant($input),
                $hash
            )),
            $crate::prelude::UpdateEntryOutput
        )
    }};
}

#[macro_export]
macro_rules! delete_cap_grant {
    ( $hash:expr ) => {{
        $crate::delete_entry!($hash)
    }};
}
