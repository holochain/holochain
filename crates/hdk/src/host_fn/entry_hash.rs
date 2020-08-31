#[macro_export]
macro_rules! entry_hash {
    ( $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __entry_hash,
                $crate::prelude::EntryHashInput::new($crate::prelude::Entry::App(sb.try_into()?)),
                $crate::prelude::EntryHashOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
