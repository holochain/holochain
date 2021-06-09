use hdk::prelude::*;

#[hdk_extern]
fn random_bytes(bytes: u32) -> ExternResult<Bytes> {
    hdk::prelude::random_bytes(bytes)
}

#[cfg(test)]
pub mod tests {
    #[test]
    fn random_bytes_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input = 1;
        let output = hdk::prelude::Bytes::from(vec![4_u8]);
        let output_closure = output.clone();
        mock_hdk.expect_random_bytes()
            .with(hdk::prelude::mockall::predicate::eq(
                input
            ))
            .times(1)
            .return_once(move |_| Ok(output_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::random_bytes(input);

        assert_eq!(
            result,
            Ok(
                output
            )
        );
    }
}