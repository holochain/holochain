//! A DNA gamut is a representation of all DNAs available in a given context.

use super::DnaVersionSpec;
use crate::prelude::*;
use std::collections::HashSet;

/// Representation of all DNAs available in a given context.
/// When given a DnaVersionSpec, a particular DNA can be selected from this
/// gamut
///
/// NB: since our DnaVersionSpec is currently very simplistic, so is the gamut.
/// As our versioning becomes more expressive, so will this type. For instance,
/// if we introduce semver, the gamut will include versions of DNAs as well.
pub struct DnaGamut(HashSet<DnaHashB64>);

/// We don't have any notion of DNA versioning other than the hash, but this is
/// a placeholder to indicate the need for it in the future and to start using
/// it in public interfaces.
pub type DnaVersion = ();

impl DnaGamut {
    /// Given a version spec, return the best match
    pub fn resolve_dna(&self, spec: DnaVersionSpec) -> Option<(DnaHashB64, DnaVersion)> {
        for hash in spec.dna_hashes() {
            if self.0.contains(hash) {
                return Some((hash.clone(), ()));
            }
        }
        return None;
    }
}

/// Possible results of DNA resolution
pub enum DnaGamutResolution {
    /// A match was found within the gamut
    Found(DnaHashB64, DnaVersion),
    /// No match was found
    NoMatch,
    /// Multiple matches were found, or other scenario that requires user
    /// intervention for resolution (TODO, placeholder)
    Conflict,
}
