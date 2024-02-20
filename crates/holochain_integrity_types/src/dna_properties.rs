//! # DNA Properties Support types

/// Trait to convert from dna properties into specified type
pub trait TryFromDnaProperties {
    /// The error associated with this conversion.
    type Error;

    /// Attempts to deserialize DNA properties into specified type
    fn try_from_dna_properties() -> Result<Self, Self::Error>
    where
        Self: Sized;
}
