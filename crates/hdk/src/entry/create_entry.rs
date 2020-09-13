#[macro_export]
macro_rules! create_entry {
    ( $input:expr ) => {{
        $crate::prelude::host_externs!(__create);

        let try_sb = $crate::prelude::SerializedBytes::try_from($input);
        match try_sb {
            Ok(sb) => create!($input, $crate::prelude::Entry::App(sb.try_into()?)),
            Err(e) => Err(e),
        }
    }};
}
