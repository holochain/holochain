#[macro_export]
macro_rules! create_cap_claim {
    ( $input:expr ) => {{
        create!(
            $crate::prelude::EntryDefId::CapClaim,
            $crate::prelude::Entry::CapClaim($input)
        )
    }};
}
