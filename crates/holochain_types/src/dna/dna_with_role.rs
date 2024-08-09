//! DNAs associated with RoleNames

use crate::prelude::*;

/// A DnaFile with a role name assigned.
///
/// This trait is implemented for both `DnaFile` and (`RoleName, DnaFile)` tuples.
/// When a test doesn't need to specify a RoleName, it can use just the DnaFile,
/// in which case an arbitrary RoleName will be assigned.
pub trait DnaWithRole: Clone + std::fmt::Debug + Sized {
    /// The associated role name
    fn role(&self) -> RoleName;

    /// The DNA
    fn dna(&self) -> &DnaFile;

    /// The DNA
    fn into_dna(self) -> DnaFile;

    /// Replace the DNA without changing the role
    fn replace_dna(self, dna: DnaFile) -> (RoleName, DnaFile) {
        (self.role(), dna)
    }
}

impl DnaWithRole for DnaFile {
    fn role(&self) -> RoleName {
        self.dna_hash().to_string()
    }

    fn dna(&self) -> &DnaFile {
        self
    }

    fn into_dna(self) -> DnaFile {
        self
    }
}

impl DnaWithRole for (RoleName, DnaFile) {
    fn role(&self) -> RoleName {
        self.0.clone()
    }

    fn dna(&self) -> &DnaFile {
        &self.1
    }

    fn into_dna(self) -> DnaFile {
        self.1
    }
}
