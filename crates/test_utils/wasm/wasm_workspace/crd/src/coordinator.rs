use crate::integrity::*;
use hdk::prelude::*;

use EntryZomes::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityCrd(EntryTypes),
}

#[hdk_extern]
fn create(_: ()) -> ExternResult<ActionHash> {
    create_entry(&IntegrityCrd(EntryTypes::Thing(Thing)))
}

/// `read` seems to be a reserved worked that causes SIGSEGV invalid memory reference when used as `#[hdk_extern]`
#[hdk_extern]
fn reed(action_hash: ActionHash) -> ExternResult<Option<Record>> {
    get(action_hash, GetOptions::latest())
}

#[hdk_extern]
fn delete_via_hash(action_hash: ActionHash) -> ExternResult<ActionHash> {
    delete_entry(action_hash)
}

#[hdk_extern]
fn delete_via_input(delete_input: DeleteInput) -> ExternResult<ActionHash> {
    delete_entry(delete_input)
}

#[cfg(all(test, feature = "mock"))]
pub mod test {
    use fixt::prelude::*;
    use hdk::prelude::*;

    #[test]
    fn create_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let thing = EntryTypes::Thing(Thing);
        let action_hash = fixt!(ActionHash);
        let closure_action_hash = action_hash.clone();
        mock_hdk
            .expect_create()
            .with(hdk::prelude::mockall::predicate::eq(CreateInput {
                input: EntryInput::App(AppEntry {
                    entry_def_index: ScopedEntryDefIndex::try_from(thing).unwrap(),
                    visibility: EntryVisibility::Public,
                    entry: thing.try_into().unwrap(),
                }),
                chain_top_ordering: Default::default(),
            }))
            .times(1)
            .return_once(move |_| Ok(closure_action_hash));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::create(());

        assert_eq!(result, Ok(action_hash))
    }

    #[test]
    fn get_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_action_hash = fixt!(ActionHash);
        mock_hdk
            .expect_get()
            .with(hdk::prelude::mockall::predicate::eq(vec![GetInput::new(
                input_action_hash.clone().into(),
                GetOptions::latest(),
            )]))
            .times(1)
            .return_once(move |_| Ok(vec![None]));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::reed(input_action_hash);

        assert_eq!(result, Ok(None))
    }

    #[test]
    fn delete_hash_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_action_hash = fixt!(ActionHash);
        let output_action_hash = fixt!(ActionHash);
        let output_action_hash_closure = output_action_hash.clone();
        mock_hdk
            .expect_delete()
            .with(hdk::prelude::mockall::predicate::eq(DeleteInput::new(
                input_action_hash.clone(),
                ChainTopOrdering::default(),
            )))
            .times(1)
            .return_once(move |_| Ok(output_action_hash_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::delete_via_hash(input_action_hash);

        assert_eq!(result, Ok(output_action_hash))
    }

    #[test]
    fn delete_input_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_action_hash = fixt!(ActionHash);
        let output_action_hash = fixt!(ActionHash);
        let output_action_hash_closure = output_action_hash.clone();
        mock_hdk
            .expect_delete()
            .with(hdk::prelude::mockall::predicate::eq(DeleteInput::new(
                input_action_hash.clone(),
                ChainTopOrdering::Relaxed,
            )))
            .times(1)
            .return_once(move |_| Ok(output_action_hash_closure));

        hdk::prelude::set_hdk(mock_hdk);

        let input = DeleteInput {
            deletes_action_hash: input_action_hash,
            chain_top_ordering: ChainTopOrdering::Relaxed,
        };
        let result = super::delete_via_input(input);

        assert_eq!(result, Ok(output_action_hash))
    }
}
