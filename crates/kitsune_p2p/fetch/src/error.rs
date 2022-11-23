/// Kitsune Fetch Error
pub enum FetchError {}

/// Kitsune Fetch Result
pub type FetchResult<T> = Result<T, FetchError>;
