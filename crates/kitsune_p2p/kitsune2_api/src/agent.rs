//! Agent-related types.

use crate::*;

/// A collection of agent metadata.
pub trait AgentInfo: 'static + Send + Sync + std::fmt::Debug {
    /// Helper required for downcast.
    fn as_any(&self) -> Arc<dyn std::any::Any + 'static + Send + Sync>;

    /// Agent identifier.
    fn id(&self) -> DynId;

    /// If `true`, this is a real, active, non-expired agent.
    /// If `false`, this agent is either expired, or offline.
    fn is_active(&self) -> bool;

    /// The timestamp at which this metadata was created.
    fn created_at(&self) -> Timestamp;

    /// The timestamp at which this agent metadata expires.
    fn expires_at(&self) -> Timestamp;

    /// The storage arq claimed by this agent.
    fn storage_arq(&self) -> arq::DynArq;
}

/// Trait-object [AgentInfo].
pub type DynAgentInfo = Arc<dyn AgentInfo>;
