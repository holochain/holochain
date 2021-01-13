#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct Bundle {
    /// Name of the bundle, just for context
    name: String,

    /// Version of bundle format
    version: u8,

    /// The Dnas that make up this bundle
    dnas: Vec<BundleDnas>,
}

pub(super) struct BundleDna {
    /// The CellNick which will be given to the installed Cell for this Dna
    nick: CellNick,

    /// The hash of the Dna
    hash: DnaHash,

    /// Optional Dna location.
    /// If None, it is assumed that the Dna with this hash is present in the bundle.
    /// If Some, specifies an external location to fetch this Dna from.
    location: Option<BundleDnaLocation>,
}

pub(super) enum BundleDnaLocation {
    /// Get Dna from local filesystem
    Local(PathBuf),

    /// Get Dna from URL
    Url(String),
}
