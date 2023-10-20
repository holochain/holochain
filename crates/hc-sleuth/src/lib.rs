// TODO: remove
#![allow(warnings)]

use std::collections::HashMap;

pub(crate) use holochain_state::prelude::*;
pub(crate) use kitsune_p2p::gossip::sharded_gossip::GossipType;

#[macro_use]
mod cause;
mod context;
mod fact;
pub use cause::*;
pub use context::*;
pub use fact::*;
pub mod query;
#[macro_use]
pub(crate) mod report;
pub use report::*;

#[cfg(test)]
pub mod test_fact;

pub mod holochain;

#[cfg(test)]
mod tests;

/// The primary significant states an item can be in from a node's perspective
pub enum ItemStatus {
    /// The item has never been encountered by the agent in any form:
    /// the hash has not been published or gossiped to this node, and this node
    /// has never attempted to fetch
    Unseen,
    /// The item exists either in the cache via a `get`, or is pending integration.
    /// The significance of this is that it will be available for `must_get_*` calls.
    Exists,
    /// The item is fully integrated under this authority type
    Integrated(DhtOpType),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Integrated(IntegrationStep),
    Cached(Cached),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrationStep {
    Propagated(Propagation),
    Fetched(Fetch),
    SysValidated(SysVal),
    AppValidated(AppVal),
    Integrated(Integrated),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cached {
    timestamp: Timestamp,
    from: AgentPubKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gossip {
    timestamp: Timestamp,
    gossip_type: GossipType,
    from: AgentPubKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publish {
    timestamp: Timestamp,
    from: AgentPubKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Propagation {
    Gossip(Gossip),
    Publish(Publish),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocker<Hash> {
    Get(GetBlocker),
    Integration(AuthorityBlocker<Hash>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityBlocker<Hash> {
    Propagation(PropagationBlocker),
    Fetch(FetchBlocker),
    SysVal(SysValBlocker<Hash>),
    AppVal(AppValBlocker<Hash>),
    Integration(IntegrationBlocker),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropagationBlocker {
    /// Gossip or publishing is not hooked up / the workflows are not running
    WorkflowNotRunning,
    // No peers have integrated this item, so nobody can send it to me
    NoPeersHaveIntegrated,
    /// There are peers who hold this item but they are not visible to me
    InaccessablePeers {
        authorities: Vec<AgentPubKey>,
    },
    /// There are agents who hold this item and I know about them, but I have not yet talked to them
    StillNoPropagation {
        authorities: Vec<AgentPubKey>,
    },
    /// Other
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GossipBlocker {
    /// Peers holding this item talk to me but they don't send me this op,
    /// maybe because they think I have it
    PeersWithholding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchBlocker {
    /// I can't connect to the source peer
    SourceInaccessible,
    /// The FetchPool is not retrying fetching after failing / the op hash is somehow stuck in the pool
    FetchPoolMalfunction,
    /// There is a loop or some degenerate case where other items fill up the fetch pool before my op hash can be processed
    FetchPoolStarvation,
    /// Other
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SysValBlocker<Hash> {
    /// The sys validation workflow is not running properly and validation is not being attempted
    WorkflowNotRunning,
    /// Dependencies are missing, and we recursively report on those dependencies
    MissingDeps(Vec<(Hash, Blocker<Hash>)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppValBlocker<Hash> {
    /// The app validation workflow is not running properly and validation is not being attempted
    WorkflowNotRunning,
    /// Wasm can't be called for some reason
    WasmFailure,
    /// Dependencies are missing, and we recursively report on those dependencies
    MissingDeps(Vec<(Hash, Blocker<Hash>)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrationBlocker {
    /// The app validation workflow is not running properly and validation is not being attempted
    WorkflowNotRunning,
}

/// Reasons why a `get` might fail
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GetBlocker {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetch {
    timestamp: Timestamp,
    from: AgentPubKey,
    propagation: Propagation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SysVal {
    timestamp: Timestamp,
    outcome: ValidationStatus,
    fetch: Fetch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppVal {
    timestamp: Timestamp,
    sys_validation: SysVal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Integrated {
    timestamp: Timestamp,
    app_validation: AppVal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionReport {
    Pass { step: Option<Step> },
    Fail { step: Option<Step> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpReport {
    Pass { step: Option<Step> },
    Fail { step: Option<Step> },
}
