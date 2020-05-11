//! Traits and types for generating "Hashed" wrappers around `TryInto<SerializedBytes>` items.

use crate::*;

/// Trait representing a type that has been hashed.
pub trait Hashed {
    /// The item that has been hashed.
    type Content: Sized + std::convert::TryInto<SerializedBytes>;

    /// The hash type used by this "Hashed" wrapper type.
    type HashType: Sized + HoloHashCoreHash;

    /// Unwrap the main item from this "Hashed" wrapper.
    fn into_inner(self) -> Self::Content;

    /// Unwrap the complete contents of this "Hashed" wrapper.
    fn into_inner_with_hash(self) -> (Self::Content, Self::HashType);

    /// Access the main item stored in this wrapper type.
    fn as_content(&self) -> &Self::Content;

    /// Access the already-calculated hash stored in this wrapper type.
    fn as_hash(&self) -> &Self::HashType;
}

/// Generic based "Hashed" struct implementation.
#[derive(Debug, Clone)]
pub struct GenericHashed<C, H>(C, H)
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash;

impl<C, H> GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    /// Produce a "Hashed" wrapper with a provided hash.
    pub fn with_pre_hashed(t: C, h: H) -> Self {
        Self(t, h)
    }
}

impl<C, H> Hashed for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    type Content = C;
    type HashType = H;

    fn into_inner(self) -> Self::Content {
        self.0
    }

    fn into_inner_with_hash(self) -> (Self::Content, Self::HashType) {
        (self.0, self.1)
    }

    fn as_content(&self) -> &Self::Content {
        &self.0
    }

    fn as_hash(&self) -> &Self::HashType {
        &self.1
    }
}

impl<C, H> std::convert::From<GenericHashed<C, H>> for (C, H)
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    fn from(g: GenericHashed<C, H>) -> (C, H) {
        g.into_inner_with_hash()
    }
}

impl<C, H> std::ops::Deref for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.as_content()
    }
}

impl<C, H> std::convert::AsRef<C> for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    fn as_ref(&self) -> &C {
        self.as_content()
    }
}

impl<C, H> std::borrow::Borrow<C> for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    fn borrow(&self) -> &C {
        self.as_content()
    }
}

impl<C, H> std::cmp::PartialEq for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    fn eq(&self, other: &Self) -> bool {
        self.as_hash() == other.as_hash()
    }
}

impl<C, H> std::cmp::Eq for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
}

impl<C, H> std::hash::Hash for GenericHashed<C, H>
where
    C: Sized + std::convert::TryInto<SerializedBytes>,
    H: Sized + HoloHashCoreHash,
{
    fn hash<StdH: std::hash::Hasher>(&self, state: &mut StdH) {
        self.as_hash().hash(state)
    }
}
