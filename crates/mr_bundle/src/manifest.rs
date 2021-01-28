use crate::location::Location;

pub trait Manifest:
    Clone + Sized + PartialEq + Eq + serde::Serialize + serde::de::DeserializeOwned
{
    fn locations(&self) -> Vec<Location>;
}
