#[macro_export]
macro_rules! create_cap_grant {
    ( $input:expr ) => {{
        $crate::prelude::host_externs!(__create_entry);

        $crate::host_fn!(
            __create_entry,
            $crate::prelude::CreateInput::new((
                $crate::prelude::EntryDefId::CapGrant,
                $crate::prelude::Entry::CapGrant($input)
            )),
            $crate::prelude::CreateOutput
        )
    }};
}

#[macro_export]
macro_rules! update_cap_grant {
    ( $hash:expr, $input:expr ) => {{
        $crate::host_fn!(
            __update_entry,
            $crate::prelude::UpdateInput::new((
                $crate::prelude::EntryDefId::CapGrant,
                $crate::prelude::Entry::CapGrant($input),
                $hash
            )),
            $crate::prelude::UpdateOutput
        )
    }};
}

#[macro_export]
macro_rules! delete_cap_grant {
    ( $hash:expr ) => {{
        $crate::delete_entry!($hash)
    }};
}
