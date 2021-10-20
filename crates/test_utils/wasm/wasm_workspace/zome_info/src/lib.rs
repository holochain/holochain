use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk::prelude::zome_info()
}

#[hdk_extern]
fn call_info(_: ()) -> ExternResult<CallInfo> {
    // Commit something here so we can show the as_at won't shift in the call
    // info returned.
    create_entry(Thing)?;
    hdk::prelude::call_info()
}

#[hdk_extern]
fn dna_info(_: ()) -> ExternResult<DnaInfo> {
    hdk::prelude::dna_info()
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

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::zome_info(());

        assert_eq!(
            result,
            Ok(
                output
            )
        );
    }
}