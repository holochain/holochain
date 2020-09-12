#[macro_export]
macro_rules! create_entry {
    ( $input:expr ) => {{
        $crate::prelude::host_externs!(__create_entry);

        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __create_entry,
                $crate::prelude::CreateEntryInput::new((
                    $input.into(),
                    $crate::prelude::Entry::App(sb.try_into()?)
                )),
                $crate::prelude::CreateEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
