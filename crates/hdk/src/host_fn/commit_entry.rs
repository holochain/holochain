#[macro_export]
macro_rules! commit_entry {
    ( $input:expr ) => {{
        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => $crate::host_fn!(
                __commit_entry,
                $crate::prelude::CommitEntryInput::new((
                    $input.into(),
                    $crate::prelude::Entry::App(sb.try_into()?)
                )),
                $crate::prelude::CommitEntryOutput
            ),
            Err(e) => Err(e),
        }
    }};
}
