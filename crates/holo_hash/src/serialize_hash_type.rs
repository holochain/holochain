use crate::HashType;
use serde::{Deserializer, Serializer};

pub fn serialize<S, T: HashType>(t: &T, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_unit_struct(t.hash_name())
}

pub fn deserialize<'de, D, T: HashType>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    d.deserialize_unit_struct(name, visitor)
}
