pub enum FetchError {}

pub type FetchResult<T> = Result<T, FetchError>;
