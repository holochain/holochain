#[macro_export]
macro_rules! create_cap_claim {
    ( $input:expr ) => {{
        $crate::prelude::host_externs!(__create_entry);

        $crate::host_fn!(
            __create_entry,
            $crate::prelude::CreateInput::new((
                $crate::prelude::EntryDefId::CapClaim,
                $crate::prelude::Entry::CapClaim($input)
            )),
            $crate::prelude::CreateOutput
        )
    }};
}
