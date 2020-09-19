/// Sign some data using the private key for the passed public key
///
/// Assuming the private key for the provided
#[macro_export]
macro_rules! sign {
    ( $key:expr, $data:expr ) => {{
        $crate::prelude::host_externs!(__sign);
        $crate::host_fn!(
            __sign,
            $crate::prelude::holochain_zome_types::zome_io::SignInput::new(
                $crate::prelude::holochain_zome_types::signature::SignInput {
                    key: $key.into(),
                    data: $data.into(),
                }
            ),
            $crate::prelude::SignOutput
        )
    }};
}
