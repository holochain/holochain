#[macro_export]
macro_rules! update_entry {
    ( $hash:expr, $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __update_entry,
                $crate::prelude::UpdateInput::new((
                    $input.into(),
                    $crate::prelude::Entry::App(sb.try_into()?),
                    $hash
                )),
                $crate::prelude::UpdateOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
