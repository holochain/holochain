//! Fetch queue types

/// Info about the fetch queue
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FetchPoolInfo {
    /// Total number of bytes expected to be received through fetches
    pub op_bytes_to_fetch: usize,

    /// Total number of ops expected to be received through fetches
    pub num_ops_to_fetch: usize,
}
