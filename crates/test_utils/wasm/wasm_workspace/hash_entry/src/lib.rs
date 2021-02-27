use hdk::prelude::*;

#[derive(Serialize, Deserialize, Debug)]
enum TemperatureUnit {
    Kelvin,
    Farenheit,
    Celcius,
}

#[hdk_entry(id="temperature")]
struct Temperature(u32, TemperatureUnit);

entry_defs![Temperature::entry_def()];

fn temperature() -> Temperature {
    Temperature(32, TemperatureUnit::Celcius)
}

#[hdk_extern]
fn twenty_three_degrees_entry_hash(_: ()) -> ExternResult<EntryHash> {
    let temp = temperature();
    let header_hash: HeaderHash = create_entry(&temp)?;
    let element: Element = get(header_hash, GetOptions::content())?.unwrap();
    match element.entry() {
        ElementEntry::Present(entry) => hdk::prelude::hash_entry(entry.clone()),
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
