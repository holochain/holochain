#[cfg(feature = "now")]
use chrono::ParseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampError {
    Overflow,
    #[cfg(feature = "now")]
    ParseError(ParseError),
    OutOfOrder,
}

pub type TimestampResult<T> = Result<T, TimestampError>;

impl std::error::Error for TimestampError {
    #[cfg(feature = "now")]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TimestampError::Overflow => None,
            TimestampError::ParseError(e) => e.source(),
            TimestampError::OutOfOrder => None,
        }
    }
}

#[cfg(feature = "now")]
impl From<ParseError> for TimestampError {
    fn from(e: ParseError) -> Self {
        Self::ParseError(e)
    }
}

impl core::fmt::Display for TimestampError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimestampError::Overflow => write!(
                f,
                "Overflow in adding, subtracting or creating from a Duration."
            ),
            #[cfg(feature = "now")]
            TimestampError::ParseError(s) => s.fmt(f),
            TimestampError::OutOfOrder => {
                write!(f, "Start was after the end of a Timestamp bounded range.")
            }
        }
    }
}
