#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct Bundle {
    /// Name of the bundle, just for context
    name: String,

    /// Version of bundle format
    version: u8,

    /// The Dnas that make up this bundle
    dnas: Vec<BundleDnas>,
}

/// Description of a Dna referenced by this Bundle
pub(super) struct BundleDna {
    /// The CellNick which will be given to the installed Cell for this Dna
    nick: CellNick,

    /// The hash of the Dna.
    ///
    /// In "dev" mode (to be defined), the hash can be omitted when installing
    /// a bundle, since it may be frequently changing. Otherwise, it is required
    /// for "real" bundles.
    hash: Option<DnaHash>,

    /// Where to find this Dna
    location: BundleDnaLocation,
}

/// Where to find this Dna.
/// If Local, the path may refer to a Dna which is bundled with the manifest,
/// or it may be to some other absolute or relative file path.
pub(super) enum BundleDnaLocation {
    /// Get Dna from local filesystem
    Local(PathBuf),

    /// Get Dna from URL
    Url(String),
}
