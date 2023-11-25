/// Subdirectory of the config directory where the conductor stores its
/// databases.
pub const DATABASES_DIRECTORY: &str = "databases";

/// Subdirectory of the config directory where the conductor stores its
/// keystore. Keep the path short so that when it's used in CI the path doesn't
/// get too long to be used as a domain socket
pub const KEYSTORE_DIRECTORY: &str = "ks";

/// Subdirectory of the config directory where the conductor stores its
/// compiled wasm.
pub const WASM_DIRECTORY: &str = "wasm";


