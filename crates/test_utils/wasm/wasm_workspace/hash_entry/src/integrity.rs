use holochain_deterministic_integrity::prelude::*;

#[derive(Serialize, Deserialize, Debug)]
pub enum TemperatureUnit {
    Kelvin,
    Farenheit,
    Celcius,
}

#[hdk_entry_helper]
pub struct Temperature(pub u32, pub TemperatureUnit);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Temperature(Temperature),
}
