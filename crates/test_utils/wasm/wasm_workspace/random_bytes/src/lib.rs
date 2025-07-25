use rand::TryRngCore;
use hdk::prelude::*;

#[hdk_extern]
fn random_bytes(bytes: u32) -> ExternResult<Bytes> {
    hdk::prelude::random_bytes(bytes)
}

#[hdk_extern]
fn rand_random_bytes(bytes: u32) -> ExternResult<Bytes> {
    let mut bytes = vec![0; bytes as usize];
    rand::rngs::OsRng.try_fill_bytes(&mut bytes[..]).map_err(|e| {
        wasm_error!(
            WasmErrorInner::Host(format!("Failed to fill bytes with random data: {}", e))
        )
    })?;
    Ok(Bytes::from(bytes))
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    #[test]
    fn random_bytes_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input = 1;
        let output = hdk::prelude::Bytes::from(vec![4_u8]);
        let output_closure = output.clone();
        mock_hdk
            .expect_random_bytes()
            .with(hdk::prelude::mockall::predicate::eq(input))
            .times(1)
            .return_once(move |_| Ok(output_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::random_bytes(input);

        assert_eq!(result, Ok(output));
    }
}
