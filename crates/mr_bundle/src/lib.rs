use error::MrBundleResult;

pub mod bundle;
pub mod error;
pub mod location;
pub mod manifest;

// mod experiment;

pub fn encode<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<Vec<u8>> {
    Ok(rmp_serde::to_vec_named(data)?)
}

pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> MrBundleResult<T> {
    Ok(rmp_serde::from_read_ref(bytes)?)
}
