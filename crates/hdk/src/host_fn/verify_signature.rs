/// Verify the passed signature and public key against the passed data
#[macro_export]
macro_rules! verify_signature {
    ( $verify_signature_input:expr ) => {{
        $crate::prelude::host_externs!(__verify_signature);
        $crate::host_fn!(
            __verify_signature,
            $verify_signature_input,
            $crate::prelude::VerifySignatureOutput
        )
    }};
    ( $key:expr, $signature:expr, $data:expr ) => {{
        $crate::verify_signature!(
            $crate::prelude::holochain_zome_types::signature::VerifySignatureInput {
                key: $key.into(),
                signature: $signature.into(),
                data: $data.into(),
            }
        )
    }};
}
