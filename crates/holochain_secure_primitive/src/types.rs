/// Errors related to the secure primitive macro.
#[derive(Debug)]
pub enum SecurePrimitiveError {
    /// We have the wrong number of bytes.
    BadSize,
}
impl std::error::Error for SecurePrimitiveError {}
impl core::fmt::Display for SecurePrimitiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurePrimitiveError::BadSize => write!(f, "Bad sized secure primitive."),
        }
    }
}
