use crate::integrity::*;
use hdk::prelude::*;
use EntryZomes::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityHashEntry(EntryTypes),
}

fn temperature() -> Temperature {
    Temperature(32, TemperatureUnit::Celcius)
}

#[hdk_extern]
fn twenty_three_degrees_entry_hash(_: ()) -> ExternResult<EntryHash> {
    let temp = temperature();
    let action_hash: ActionHash = create_entry(&IntegrityHashEntry(EntryTypes::Temperature(temp)))?;
    let record: Record = get(action_hash, GetOptions::content())?.unwrap();
    match record.entry() {
        RecordEntry::Present(entry) => hdk::prelude::hash_entry(entry.clone()),
        _ => unreachable!(),
    }
}

#[hdk_extern]
fn twenty_three_degrees_hash(_: ()) -> ExternResult<EntryHash> {
    hdk::prelude::hash_entry(&temperature())
}

#[hdk_extern]
fn hash_entry(entry: Entry) -> ExternResult<EntryHash> {
    hdk::prelude::hash_entry(entry)
}

#[cfg(all(test, feature = "mock"))]
pub mod tests {
    use fixt::prelude::*;
    use hdk::prelude::*;

    #[test]
    fn hash_entry_smoke() {
        let mut mock_hdk = hdk::prelude::MockHdkT::new();

        let input_entry = fixt!(Entry);
        let output_hash = fixt!(EntryHash);
        let output_hash_closure = output_hash.clone();
        mock_hdk
            .expect_hash()
            .with(hdk::prelude::mockall::predicate::eq(HashInput::Entry(
                input_entry.clone(),
            )))
            .times(1)
            .return_once(move |_| Ok(HashOutput::Entry(output_hash_closure)));

        hdk::prelude::set_hdk(mock_hdk);

        let result = super::hash_entry(input_entry);

        assert_eq!(result, Ok(output_hash))
    }
}
