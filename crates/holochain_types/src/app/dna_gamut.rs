//! A DNA gamut is a representation of all DNAs available in a given context.

use super::DnaVersionSpec;
use crate::prelude::*;
use std::collections::{hash_map, HashMap, HashSet};

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
///
/// This type basically exists as an abstract adapter between the conductor's
/// DNA store and the app installation process. Without needing to know exactly
/// what we will need from the DNA store, we can define what questions we will
/// need to ask of it through this type.
pub struct DnaGamut(HashMap<DnaHash, HashSet<AgentPubKey>>);

/// We don't have any notion of DNA versioning other than the hash, but this is
/// a placeholder to indicate the need for it in the future and to start using
/// it in public interfaces.
pub struct DnaVersion;

impl DnaGamut {
    /// Constructor. Restructure a list of CellIds into the proper format.
    pub fn new<I: IntoIterator<Item = CellId>>(cells: I) -> Self {
        let mut map: HashMap<DnaHash, HashSet<AgentPubKey>> = HashMap::new();
        for cell in cells {
            let (dna, agent) = cell.into_dna_and_agent();
            match map.entry(dna) {
                hash_map::Entry::Occupied(mut e) => {
                    e.get_mut().insert(agent);
                }
                hash_map::Entry::Vacant(e) => {
                    e.insert(vec![agent].into_iter().collect());
                }
            }
        }
        Self(map)
    }

    #[deprecated = "Stop using the placeholder"]
    #[allow(missing_docs)]
    pub fn placeholder() -> Self {
        Self::new(std::iter::empty())
    }

    /// Given a version spec, return the best-matching DNA in the gamut
    pub fn resolve_dna(&self, spec: DnaVersionSpec) -> DnaResolution {
        for hash in spec.dna_hashes() {
            if self.0.contains_key(hash.as_ref()) {
                return DnaResolution::Match(hash.clone(), DnaVersion);
            }
        }
        DnaResolution::NoMatch
    }

    /// Given a version spec, return the best-matching CellId
    // TODO: use DPKI to filter Cells which belong to Agents that are not
    //       associated with the provided agent
    pub fn resolve_cell(&self, spec: DnaVersionSpec, _agent: &AgentPubKey) -> CellResolution {
        for hash in spec.dna_hashes() {
            if let Some(agent) = self
                .0
                .get(hash.as_ref())
                // TODO: this is where an agent check could go, but for now we
                //       just return the first one available
                .map(|agents| agents.iter().next())
                .unwrap_or(None)
            {
                return CellResolution::Match(
                    CellId::new(hash.clone().into(), agent.clone()),
                    DnaVersion,
                );
            }
        }
        CellResolution::NoMatch
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
