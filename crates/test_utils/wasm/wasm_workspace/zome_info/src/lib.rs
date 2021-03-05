use hdk::prelude::*;

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk::prelude::zome_info()
}

#[cfg(test)]
pub mod tests {
    use hdk::prelude::*;
    use ::fixt::prelude::*;

    #[test]
    fn zome_info_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let output = fixt!(ZomeInfo);
        let output_closure = output.clone();
        mock_hdk.expect_zome_info()
            .with(hdk::prelude::mockall::predicate::eq(()))
            .times(1)
            .return_once(move |_| Ok(output_closure));

        hdk::prelude::set_global_hdk(mock_hdk).unwrap();

        let result = super::zome_info(());

        assert_eq!(
            result,
            Ok(
                output
            )
        );
    }
}