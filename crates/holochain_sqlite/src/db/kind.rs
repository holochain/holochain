use holo_hash::DnaHash;
use holochain_zome_types::cell::CellId;
use kitsune_p2p_bin_data::KitsuneSpace;
use std::path::PathBuf;
use std::sync::Arc;

/// The various types of database, used to specify the list of databases to initialize
#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
pub enum DbKind {
    /// Specifies the environment used for authoring data by all cells on the same [`DnaHash`].
    #[display(fmt = "{:?}-{:?}", "_0.dna_hash()", "_0.agent_pubkey()")]
    Authored(Arc<CellId>),
    /// Specifies the environment used for dht data by all cells on the same [`DnaHash`].
    #[display(fmt = "{:?}", "_0")]
    Dht(Arc<DnaHash>),
    /// Specifies the environment used by each Cache (one per dna).
    #[display(fmt = "{:?}", "_0")]
    Cache(Arc<DnaHash>),
    /// Specifies the environment used by a Conductor
    Conductor,
    /// Specifies the environment used to save wasm
    Wasm,
    /// State of the p2p network (one per space).
    #[display(fmt = "agent_store-{:?}", "_0")]
    P2pAgentStore(Arc<KitsuneSpace>),
    /// Metrics for peers on p2p network (one per space).
    #[display(fmt = "metrics-{:?}", "_0")]
    P2pMetrics(Arc<KitsuneSpace>),
    #[cfg(feature = "test_utils")]
    Test(String),
}

pub trait DbKindT: Clone + std::fmt::Debug + Send + Sync + 'static {
    fn kind(&self) -> DbKind;

    /// Constuct a partial Path based on the kind
    fn filename(&self) -> PathBuf {
        let mut path = self.filename_inner();
        path.set_extension("sqlite3");
        path
    }

    /// The above provided `filename` method attaches the .sqlite3 extension.
    /// Implement this to provide the front part of the database filename.
    fn filename_inner(&self) -> PathBuf;

    /// Whether to wipe the database if it is corrupt.
    /// Some database it's safe to wipe them if they are corrupt because
    /// they can be refilled from the network. Other databases cannot
    /// be refilled and some manual intervention is required.
    fn if_corrupt_wipe(&self) -> bool;
}

pub trait DbKindOp {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used for authoring data by all cells on the same [`DnaHash`].
pub struct DbKindAuthored(pub Arc<CellId>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used for dht data by all cells on the same [`DnaHash`].
pub struct DbKindDht(pub Arc<DnaHash>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used by each Cache (one per dna).
pub struct DbKindCache(pub Arc<DnaHash>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used by a Conductor
pub struct DbKindConductor;

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Specifies the environment used to save wasm
pub struct DbKindWasm;

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// State of the p2p network (one per space).
pub struct DbKindP2pAgents(pub Arc<KitsuneSpace>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
/// Metrics for peers on p2p network (one per space).
pub struct DbKindP2pMetrics(pub Arc<KitsuneSpace>);

impl DbKindT for DbKindAuthored {
    fn kind(&self) -> DbKind {
        DbKind::Authored(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        [
            "authored",
            &format!("{}-{}", self.0.dna_hash(), self.0.agent_pubkey()),
        ]
        .iter()
        .collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}

impl DbKindOp for DbKindAuthored {}

impl DbKindAuthored {
    pub fn dna_hash(&self) -> &DnaHash {
        self.0.dna_hash()
    }
}

impl DbKindT for DbKindDht {
    fn kind(&self) -> DbKind {
        DbKind::Dht(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["dht", &format!("{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}

impl DbKindOp for DbKindDht {}

impl DbKindDht {
    pub fn dna_hash(&self) -> &DnaHash {
        &self.0
    }
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.0.clone()
    }
}

impl DbKindT for DbKindCache {
    fn kind(&self) -> DbKind {
        DbKind::Cache(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["cache", &format!("{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}

impl DbKindCache {
    pub fn dna_hash(&self) -> &DnaHash {
        &self.0
    }
    pub fn to_dna_hash(&self) -> Arc<DnaHash> {
        self.0.clone()
    }
}

impl DbKindOp for DbKindCache {}

impl DbKindT for DbKindConductor {
    fn kind(&self) -> DbKind {
        DbKind::Conductor
    }

    fn filename_inner(&self) -> PathBuf {
        ["conductor", "conductor"].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}

impl DbKindT for DbKindWasm {
    fn kind(&self) -> DbKind {
        DbKind::Wasm
    }

    fn filename_inner(&self) -> PathBuf {
        ["wasm", "wasm"].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        false
    }
}

impl DbKindT for DbKindP2pAgents {
    fn kind(&self) -> DbKind {
        DbKind::P2pAgentStore(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["p2p", &format!("agent_store-{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}

impl DbKindT for DbKindP2pMetrics {
    fn kind(&self) -> DbKind {
        DbKind::P2pMetrics(self.0.clone())
    }

    fn filename_inner(&self) -> PathBuf {
        ["p2p", &format!("metrics-{}", self.0)].iter().collect()
    }

    fn if_corrupt_wipe(&self) -> bool {
        true
    }
}
