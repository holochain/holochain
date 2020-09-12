#[macro_export]
macro_rules! hash_entry {
    ( $input:expr ) => {{
        $crate::prelude::host_externs!(__hash_entry);

        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __hash_entry,
                $crate::prelude::HashEntryInput::new($crate::prelude::Entry::App(sb.try_into()?)),
                $crate::prelude::HashEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
