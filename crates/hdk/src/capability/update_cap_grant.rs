#[macro_export]
macro_rules! update_cap_grant {
    ( $hash:expr, $input:expr ) => {{
        update!(
            $hash,
            $crate::prelude::EntryDefId::CapGrant,
            $crate::prelude::Entry::CapGrant($input)
        )
    }};
}
