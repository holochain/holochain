use chrono::ParseError;

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum TimestampError {
    #[error("Overflow in adding/subtracting a Duration")]
    Overflow,
    #[error(transparent)]
    ParseError(#[from] ParseError),
}

pub type TimestampResult<T> = Result<T, TimestampError>;
