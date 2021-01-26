use crate::location::Location;

pub trait BundleManifest: serde::Serialize + serde::de::DeserializeOwned {
    fn locations(&self) -> Vec<Location>;
}
