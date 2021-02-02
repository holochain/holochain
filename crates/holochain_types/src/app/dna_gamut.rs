//! A DNA gamut is a representation of all DNAs available in a given context.

use super::DnaVersionSpec;
use crate::prelude::*;
use std::collections::{HashMap, HashSet};

/// Representation of all DNAs and Cells available in a given context.
/// When given a DnaVersionSpec, a particular DNA can be selected from this
/// gamut.
///
/// Moreover, each DNA hash has associated with it a list of Agents. Each agent
/// represents a Cell which exists on the conductor, using that DNA and agent
/// pair. A DNA with no agents listed is simply registered but does not exist
/// in any Cell.
///
/// NB: since our DnaVersionSpec is currently very simplistic, so is the gamut.
/// As our versioning becomes more expressive, so will this type. For instance,
/// if we introduce semver, the gamut will include versions of DNAs as well.
pub struct DnaGamut(HashMap<DnaHashB64, HashSet<AgentPubKey>>);

/// We don't have any notion of DNA versioning other than the hash, but this is
/// a placeholder to indicate the need for it in the future and to start using
/// it in public interfaces.
pub struct DnaVersion;

impl DnaGamut {
    /// Given a version spec, return the best-matching DNA in the gamut
    pub fn resolve_dna(&self, spec: DnaVersionSpec) -> DnaResolution {
        for hash in spec.dna_hashes() {
            if self.0.contains(hash) {
                return DnaResolution::Match(hash.clone(), DnaVersion);
            }
        }
        return DnaResolution::NoMatch;
    }

    /// Given a version spec, return the best-matching CellId
    // TODO: use DPKI to filter Cells which belong to Agents that are not
    //       associated with the provided agent
    pub fn resolve_cell(&self, spec: DnaVersionSpec, _agent: &AgentPubKey) -> CellResolution {
        for hash in spec.dna_hashes() {
            if self
                .0
                .get(hash)
                // TODO: this is where an agent check could go
                .map(|agents| true)
                .unwrap_or(false)
            {
                return CellResolution::Match(hash.clone(), DnaVersion);
            }
        }
        return CellResolution::NoMatch;
    }
}

/// Possible results of DNA resolution
pub enum DnaResolution {
    /// A match was found within the gamut
    Match(DnaHashB64, DnaVersion),
    /// No match was found
    NoMatch,
    /// Multiple matches were found, or other scenario that requires user
    /// intervention for resolution (TODO, placeholder)
    Conflict,
}

/// Possible results of Cell resolution
pub enum CellResolution {
    /// A match was found within the gamut
    Match(CellId, DnaVersion),
    /// No match was found
    NoMatch,
    /// Multiple matches were found, or other scenario that requires user
    /// intervention for resolution (TODO, placeholder)
    Conflict,
}
