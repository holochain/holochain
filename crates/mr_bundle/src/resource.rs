use serde::{de::DeserializeOwned, Serialize};

pub trait Resource: Clone + Serialize + DeserializeOwned {}
impl<T> Resource for T where T: Clone + Serialize + DeserializeOwned {}
