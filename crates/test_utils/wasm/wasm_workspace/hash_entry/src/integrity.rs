use hdi::prelude::*;

#[derive(Serialize, Deserialize, Debug)]
pub enum TemperatureUnit {
    Kelvin,
    Farenheit,
    Celcius,
}

#[hdi_entry_helper]
pub struct Temperature(pub u32, pub TemperatureUnit);

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Temperature(Temperature),
}
