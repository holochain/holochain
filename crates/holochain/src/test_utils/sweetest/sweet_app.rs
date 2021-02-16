use super::SweetCell;
use holo_hash::AgentPubKey;
use holochain_types::app::InstalledAppId;
use itertools::traits::HomogeneousTuple;
use itertools::Itertools;

/// An installed app, with prebuilt SweetCells
#[derive(Clone)]
pub struct SweetApp {
    installed_app_id: InstalledAppId,
    cells: Vec<SweetCell>,
}

impl SweetApp {
    /// Constructor
    pub(super) fn new(installed_app_id: InstalledAppId, cells: Vec<SweetCell>) -> Self {
        // Ensure that all Agents are the same
        assert!(cells.iter().map(|c| c.agent_pubkey()).dedup().count() == 1);
        Self {
            installed_app_id,
            cells,
        }
    }

    /// Accessor
    pub fn installed_app_id(&self) -> &InstalledAppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn cells(&self) -> &Vec<SweetCell> {
        &self.cells
    }

    /// Accessor
    pub fn into_cells(self) -> Vec<SweetCell> {
        self.cells
    }

    /// Returns the AgentPubKey associated with this app.
    /// All Cells in this app will have the same Agent, so we just return the first.
    pub fn agent(&self) -> &AgentPubKey {
        self.cells[0].agent_pubkey()
    }
}

/// A collection of installed apps
#[derive(
    Clone, derive_more::From, derive_more::Into, derive_more::AsRef, derive_more::IntoIterator,
)]
pub struct SweetAppBatch(pub(super) Vec<SweetApp>);

impl SweetAppBatch {
    /// Get the underlying data
    pub fn into_inner(self) -> Vec<SweetApp> {
        self.0
    }

    /// Helper to destructure the nested cell data as nested tuples.
    /// The outer tuple contains the apps, the inner layer contains the cells in each app.
    ///
    /// Each level of nesting can contain 1-4 items, i.e. up to 4 apps with 4 DNAs each.
    /// Beyond 4, and this will PANIC! (But it's just for tests so it's fine.)
    pub fn into_tuples<Outer, Inner>(self) -> Outer
    where
        Outer: HomogeneousTuple<Item = Inner>,
        Inner: HomogeneousTuple<Item = SweetCell>,
        Outer::Buffer: std::convert::AsRef<[Option<Inner>]>,
        Outer::Buffer: std::convert::AsMut<[Option<Inner>]>,
        Inner::Buffer: std::convert::AsRef<[Option<SweetCell>]>,
        Inner::Buffer: std::convert::AsMut<[Option<SweetCell>]>,
    {
        self.into_inner()
            .into_iter()
            .map(|a| {
                a.into_cells()
                    .into_iter()
                    .collect_tuple::<Inner>()
                    .expect("Can't destructure more than 4 DNAs")
            })
            .collect_tuple::<Outer>()
            .expect("Can't destructure more than 4 Agents")
    }

    /// Access all Cells across all Apps, with Cells from the same App being contiguous
    pub fn cells_flattened(&self) -> Vec<&SweetCell> {
        self.0.iter().flat_map(|app| app.cells().iter()).collect()
    }

    /// Get the underlying data
    pub fn iter(&self) -> impl Iterator<Item = &SweetApp> {
        self.0.iter()
    }
}
