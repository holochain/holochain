use error::MrBundleResult;

mod bundle;
// mod compress;
pub mod error;
mod location;
mod manifest;
mod resource;

pub use bundle::Bundle;
pub use location::Location;
pub use manifest::Manifest;
pub use resource::ResourceBytes;

pub fn encode<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<Vec<u8>> {
    Ok(rmp_serde::to_vec_named(data)?)
}

pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> MrBundleResult<T> {
    Ok(rmp_serde::from_read_ref(bytes)?)
}
