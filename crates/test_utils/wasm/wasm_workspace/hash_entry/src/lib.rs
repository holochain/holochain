use hdk3::prelude::*;

#[derive(Serialize, Deserialize)]
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
        ElementEntry::Present(entry) => Ok(hdk3::prelude::hash_entry(entry.clone())?),
        _ => unreachable!(),
    }
}

#[hdk_extern]
fn twenty_three_degrees_hash(_: ()) -> ExternResult<EntryHash> {
    Ok(hdk3::prelude::hash_entry(&temperature())?)
}

#[hdk_extern]
fn hash_entry(input: HashEntryInput) -> ExternResult<EntryHash> {
    Ok(hdk3::prelude::hash_entry(input.into_inner())?)
}
