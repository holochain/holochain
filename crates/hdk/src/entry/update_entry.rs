#[macro_export]
macro_rules! update_entry {
    ( $hash:expr, $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => update!(
                $hash,
                $input.into(),
                $crate::prelude::Entry::App(sb.try_into()?)
            ),
            Err(e) => Err(e),
        }
    }};
}
