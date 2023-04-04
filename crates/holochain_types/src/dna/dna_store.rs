use std::collections::HashMap;

use holo_hash::DnaHash;

use super::DnaFile;

/// A store of DnaFiles which can be accessed by DnaHash.
pub trait DnaStore {
    /// Get the DNA for a given hash
    fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile>;
}

impl DnaStore for HashMap<DnaHash, DnaFile> {
    fn get_dna(&self, dna_hash: &DnaHash) -> Option<DnaFile> {
        // TODO: remove clone
        self.get(dna_hash).cloned()
    }
}
