use crate::location::Location;

pub trait Manifest: Clone + Sized + serde::Serialize + serde::de::DeserializeOwned {
    fn locations(&self) -> Vec<Location>;
}
