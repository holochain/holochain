use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn create(_: ()) -> ExternResult<HeaderHash> {
    create_entry(&Thing)
}

/// `read` seems to be a reserved worked that causes SIGSEGV invalid memory reference when used as `#[hdk_extern]`
#[hdk_extern]
fn reed(header_hash: HeaderHash) -> ExternResult<Option<Element>> {
    get(header_hash, GetOptions::latest())
}

#[hdk_extern]
fn delete(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    delete_entry(header_hash)
}

#[cfg(test)]
pub mod test {
    use hdk::prelude::*;
    use ::fixt::prelude::*;

    #[test]
    fn create_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let header_hash = fixt!(HeaderHash);
        let closure_header_hash = header_hash.clone();
        mock_hdk.expect_create()
            .with(hdk::prelude::mockall::predicate::eq(
                EntryWithDefId::try_from(&super::Thing).unwrap()
            ))
            .times(1)
            .return_once(move |_| Ok(closure_header_hash));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::create(());

        assert_eq!(
            result,
            Ok(
                header_hash
            )
        )
    }

    #[test]
    fn get_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_header_hash = fixt!(HeaderHash);
        mock_hdk.expect_get()
            .with(hdk::prelude::mockall::predicate::eq(
                vec![GetInput::new(input_header_hash.clone().into(), GetOptions::latest())]
            ))
            .times(1)
            .return_once(move |_| vec![Ok(None)]);

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::reed(input_header_hash);

        assert_eq!(
            result,
            Ok(
                None
            )
        )
    }

    #[test]
    fn delete_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_header_hash = fixt!(HeaderHash);
        let output_header_hash = fixt!(HeaderHash);
        let output_header_hash_closure = output_header_hash.clone();
        mock_hdk.expect_delete()
            .with(hdk::prelude::mockall::predicate::eq(
                input_header_hash.clone()
            ))
            .times(1)
            .return_once(move |_| Ok(output_header_hash_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::delete(input_header_hash);

        assert_eq!(
            result,
            Ok(
                output_header_hash
            )
        )
    }
}