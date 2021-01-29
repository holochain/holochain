use crate::{error::MrBundleResult, ResourceBytes};
use std::path::{Path, PathBuf};

/// Where to find a file.
///
/// This representation, with named fields, is chosen so that in the yaml config
/// either "path", "url", or "bundled" can be specified due to this field
/// being flattened.
#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Location {
    /// Expect file to be part of this bundle
    Bundled(PathBuf),

    /// Get file from local filesystem (not bundled)
    Path(PathBuf),

    /// Get file from URL
    Url(String),
}

pub(crate) async fn resolve_local(path: &Path) -> MrBundleResult<ResourceBytes> {
    Ok(std::fs::read(path)?)
}

pub(crate) async fn resolve_remote(url: &str) -> MrBundleResult<ResourceBytes> {
    Ok(reqwest::get(url)
        .await?
        .bytes()
        .await?
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests {

    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct TunaSalad {
        celery: Vec<Location>,

        #[serde(flatten)]
        mayo: Location,
    }

    /// Test that Location serializes in a convenient way suitable for
    /// human-readable manifests, e.g. YAML
    ///
    /// The YAML produced by this test looks like:
    /// ---
    /// celery:
    ///   - bundled: b
    ///   - path: p
    /// url: "http://r.co"
    #[test]
    fn location_flattening() {
        use serde_yaml::Value;

        let r = TunaSalad {
            celery: vec![Location::Bundled("b".into()), Location::Path("p".into())],
            mayo: Location::Url("http://r.co".into()),
        };
        let val = serde_yaml::to_value(&r).unwrap();
        println!("yaml produced:\n{}", serde_yaml::to_string(&r).unwrap());

        assert_eq!(val["celery"][0]["bundled"], Value::from("b"));
        assert_eq!(val["celery"][1]["path"], Value::from("p"));
        assert_eq!(val["url"], Value::from("http://r.co"));
    }
}
