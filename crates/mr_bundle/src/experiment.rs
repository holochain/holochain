use serde::{de::DeserializeOwned, Deserialize, Serialize};

trait Trait<'d>: Sized + Serialize + Deserialize<'d> {}

#[derive(Serialize, Deserialize)]
struct Struct<T>(T);
