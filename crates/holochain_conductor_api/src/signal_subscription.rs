use holochain_serialized_bytes::prelude::*;
use holochain_types::app::InstalledAppId;
use holochain_zome_types::cell::CellId;
use std::collections::HashMap;

/// Declares updated Signal subscription settings for an App.
/// This message is part of the AppInterfaceApi
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct SignalSubscription {
    /// The app for which to manage subscription
    installed_app_id: InstalledAppId,
    /// Fine-grained per-cell filters
    filters: SignalFilterSet,
}

/// Associate a SignalFilter with each Cell in an App.
/// The filtering can be interpreted as inclusive or exclusive,
/// depending on the use case.
///
/// An empty Exclude filter means "allow all signals" (subscribe to all).
/// An empty Include filter means "block all signals" (unsubscribe from all).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub enum SignalFilterSet {
    /// Only allow signals from the specified Cells with the specified filters,
    /// block everything else
    Include(HashMap<CellId, SignalFilter>),
    /// Only block signals from the specified Cells with the specified filters
    /// allow everything else
    Exclude(HashMap<CellId, SignalFilter>),
}

impl Default for SignalFilterSet {
    fn default() -> Self {
        // The default is no filter
        Self::allow_all()
    }
}

impl SignalFilterSet {
    /// Allow all signals to come through (subscribe to all)
    pub fn allow_all() -> Self {
        SignalFilterSet::Exclude(HashMap::new())
    }

    /// Block all signals (unsubscribe from all)
    pub fn block_all() -> Self {
        SignalFilterSet::Include(HashMap::new())
    }
}

/// Specifies fine-grained filter controls for the signals
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct SignalFilter;

impl Default for SignalFilter {
    fn default() -> Self {
        // The default is no filter
        Self::empty()
    }
}

impl SignalFilter {
    /// A passthrough filter which filters nothing
    pub fn empty() -> Self {
        SignalFilter
    }
}
