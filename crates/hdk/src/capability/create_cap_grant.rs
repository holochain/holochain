#[macro_export]
macro_rules! create_cap_grant {
    ( $input:expr ) => {{
        create!(
            $crate::prelude::EntryDefId::CapGrant,
            $crate::prelude::Entry::CapGrant($input)
        )
    }};
}
