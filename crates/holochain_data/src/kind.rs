//! Database kind types for identifying different database instances.
//!
//! This module provides typed identifiers for the different kinds of databases
//! used in Holochain. Each kind implements [`DatabaseIdentifier`] to provide
//! a unique string identifier for the database file.

use crate::DatabaseIdentifier;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_zome_types::cell::CellId;
use std::sync::Arc;

/// Specifies the database used for authoring data by a specific cell.
///
/// Each cell (DNA/Agent pair) has its own authored database.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Authored {
    cell_id: Arc<CellId>,
    id: String,
}

impl DatabaseIdentifier for Authored {
    fn database_id(&self) -> &str {
        &self.id
    }
}

impl Authored {
    /// Create a new authored database identifier for a cell.
    pub fn new(cell_id: Arc<CellId>) -> Self {
        let id = format!("authored-{}-{}", cell_id.dna_hash(), cell_id.agent_pubkey());
        Self { cell_id, id }
    }

    /// Get the DNA hash for this authored database.
    pub fn dna_hash(&self) -> &DnaHash {
        self.cell_id.dna_hash()
    }

    /// Get the agent public key for this authored database.
    pub fn agent_pubkey(&self) -> &AgentPubKey {
        self.cell_id.agent_pubkey()
    }
}

/// Specifies the database used for DHT data for a specific DNA.
///
/// All cells on the same DNA share the same DHT database.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dht {
    dna_hash: Arc<DnaHash>,
    id: String,
}

impl DatabaseIdentifier for Dht {
    fn database_id(&self) -> &str {
        &self.id
    }
}

impl Dht {
    /// Create a new DHT database identifier for a DNA.
    pub fn new(dna_hash: Arc<DnaHash>) -> Self {
        let id = format!("dht-{}", dna_hash);
        Self { dna_hash, id }
    }

    /// Get the DNA hash for this DHT database.
    pub fn dna_hash(&self) -> &DnaHash {
        &self.dna_hash
    }

    /// Get an Arc reference to the DNA hash.
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.dna_hash.clone()
    }
}

/// Specifies the database used by the Conductor.
///
/// There is only one conductor database per Holochain instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Conductor;

impl DatabaseIdentifier for Conductor {
    fn database_id(&self) -> &str {
        "conductor"
    }
}

/// Specifies the database used to save Wasm code.
///
/// There is only one wasm database per Holochain instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Wasm;

impl DatabaseIdentifier for Wasm {
    fn database_id(&self) -> &str {
        "wasm"
    }
}

/// Database for storing peer metadata.
///
/// Each DNA has its own peer metadata database.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerMetaStore {
    dna_hash: Arc<DnaHash>,
    id: String,
}

impl DatabaseIdentifier for PeerMetaStore {
    fn database_id(&self) -> &str {
        &self.id
    }
}

impl PeerMetaStore {
    /// Create a new peer metadata database identifier for a DNA.
    pub fn new(dna_hash: Arc<DnaHash>) -> Self {
        let id = format!("p2p-peer-meta-{}", dna_hash);
        Self { dna_hash, id }
    }

    /// Get the DNA hash for this peer metadata database.
    pub fn dna_hash(&self) -> &DnaHash {
        &self.dna_hash
    }

    /// Get an Arc reference to the DNA hash.
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.dna_hash.clone()
    }
}
