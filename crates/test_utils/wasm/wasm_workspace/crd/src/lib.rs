use hdk::prelude::*;

#[hdk_entry(id = "thing")]
struct Thing;

entry_defs![Thing::entry_def()];

#[hdk_extern]
fn xcreate(_: ()) -> ExternResult<HeaderHash> {
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
    /// Wrapper test to serialize the tests so they don't overwrite the global concurrently.
    fn smokes() {
        create_smoke();
        get_smoke();
    }

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

        hdk::prelude::set_global_hdk(mock_hdk).unwrap();

        let result = super::xcreate(());

        assert_eq!(
            result,
            Ok(
                header_hash
            )
        )
    }

    fn get_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_header_hash = fixt!(HeaderHash);
        mock_hdk.expect_get()
            .with(hdk::prelude::mockall::predicate::eq(
                GetInput::new(input_header_hash.clone().into(), GetOptions::latest())
            ))
            .times(1)
            .return_once(move |_| Ok(None));

        hdk::prelude::set_global_hdk(mock_hdk).unwrap();

        let result = super::reed(input_header_hash);

        assert_eq!(
            result,
            Ok(
                None
            )
        )
    }
}