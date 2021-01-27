use crate::location::Location;

pub trait BundleManifest: Clone + Sized + serde::Serialize + serde::de::DeserializeOwned {
    fn locations(&self) -> Vec<Location>;
}
