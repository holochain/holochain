#[macro_export]
macro_rules! update_entry {
    ( $hash:expr, $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __update_entry,
                $crate::prelude::UpdateEntryInput::new((
                    $input.into(),
                    $crate::prelude::Entry::App(sb),
                    $hash
                )),
                $crate::prelude::UpdateEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
