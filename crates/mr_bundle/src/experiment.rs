use serde::{de::DeserializeOwned, Deserialize, Serialize};

trait Manifest: Sized + Serialize + DeserializeOwned {}

#[derive(Serialize, Deserialize)]
#[serde(from = "BundleSerialized", into = "BundleSerialized")]
struct Bundle<M: Manifest>(M);

#[derive(Serialize, Deserialize)]
struct BundleSerialized(Vec<u8>);
