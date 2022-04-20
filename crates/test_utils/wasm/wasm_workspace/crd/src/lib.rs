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
fn delete_via_hash(header_hash: HeaderHash) -> ExternResult<HeaderHash> {
    delete_entry(header_hash)
}

#[hdk_extern]
fn delete_via_input(delete_input: DeleteInput) -> ExternResult<HeaderHash> {
    delete_entry(delete_input)
}

#[cfg(all(test, feature = "mock"))]
pub mod test {
    use ::fixt::prelude::*;
    use hdk::prelude::*;

    #[test]
    fn create_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let header_hash = fixt!(HeaderHash);
        let closure_header_hash = header_hash.clone();
        mock_hdk
            .expect_create()
            .with(hdk::prelude::mockall::predicate::eq(CreateInput {
                entry_def_id: super::Thing::entry_def_id(),
                entry: super::Thing.try_into().unwrap(),
                chain_top_ordering: Default::default(),
            }))
            .times(1)
            .return_once(move |_| Ok(closure_header_hash));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::create(());

        assert_eq!(result, Ok(header_hash))
    }

    #[test]
    fn get_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_header_hash = fixt!(HeaderHash);
        mock_hdk
            .expect_get()
            .with(hdk::prelude::mockall::predicate::eq(vec![GetInput::new(
                input_header_hash.clone().into(),
                GetOptions::latest(),
            )]))
            .times(1)
            .return_once(move |_| Ok(vec![None]));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::reed(input_header_hash);

        assert_eq!(result, Ok(None))
    }

    #[test]
    fn delete_hash_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_header_hash = fixt!(HeaderHash);
        let output_header_hash = fixt!(HeaderHash);
        let output_header_hash_closure = output_header_hash.clone();
        mock_hdk
            .expect_delete()
            .with(hdk::prelude::mockall::predicate::eq(DeleteInput::new(
                input_header_hash.clone(),
                ChainTopOrdering::default(),
            )))
            .times(1)
            .return_once(move |_| Ok(output_header_hash_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::delete_via_hash(input_header_hash);

        assert_eq!(result, Ok(output_header_hash))
    }

    #[test]
    fn delete_input_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_header_hash = fixt!(HeaderHash);
        let output_header_hash = fixt!(HeaderHash);
        let output_header_hash_closure = output_header_hash.clone();
        mock_hdk
            .expect_delete()
            .with(hdk::prelude::mockall::predicate::eq(DeleteInput::new(
                input_header_hash.clone(),
                ChainTopOrdering::Relaxed,
            )))
            .times(1)
            .return_once(move |_| Ok(output_header_hash_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let input = DeleteInput {
            deletes_header_hash: input_header_hash,
            chain_top_ordering: ChainTopOrdering::Relaxed,
        };
        let result = super::delete_via_input(input);

        assert_eq!(result, Ok(output_header_hash))
    }
}
