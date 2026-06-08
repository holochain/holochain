//! Requester-side helpers for `must_get_agent_activity`.
//!
//! The DHT-read agent-activity logic (classifying an author's chain into
//! valid/rejected, computing chain status and highest observed, attaching
//! warrants) now lives in `holochain_state`'s `DhtStore`. What remains here is
//! the requester-side machinery for merging `must_get_agent_activity`
//! responses received from multiple peers across the network.

pub mod must_get_agent_activity;
