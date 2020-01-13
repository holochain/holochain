//! Just enough to get us rolling for now

pub type SkunkError = String;

pub type SkunkResult<T> = Result<T, SkunkError>;