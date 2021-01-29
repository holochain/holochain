use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::error::MrBundleResult;

pub type ResourceBytes = Vec<u8>;

// #[derive(Serialize, Deserialize)]
// pub enum Resource {
//     Raw(Vec<u8>),
//     Serialized(Vec<u8>),
// }

// impl Resource {
//     pub fn raw(bytes: Vec<u8>) -> Self {
//         Self::Raw(bytes)
//     }

//     pub fn structured<R: ResourceData>(data: &R) -> MrBundleResult<Self> {
//         Ok(Self::Serialized(crate::encode(data)?))
//     }
// }

pub trait ResourceData: Clone + Serialize + DeserializeOwned {}
impl<T> ResourceData for T where T: Clone + Serialize + DeserializeOwned {}
