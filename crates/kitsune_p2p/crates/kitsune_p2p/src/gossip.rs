//! Various gossip strategies for Kitsune.
//!
//! Gossip is one of the main methods for nodes to exchange data in a Kitsune network.
//! During a gossip round, there are two things gossiped about:
//! info about other Agents in the network, and Ops, which are opaque chunks of data
//! used by the user of Kitsune.
//!
//! There are two types of gossip, Recent and Historical.
//!
//! Recent gossip is covers the last N minutes (currently N =  5) of Op activity. It uses bloom filters
//! to convey which ops are held and to discover which ops need to be transmitted during this round.
//! It is "pessimistic" in that we *a priori* expect our gossip partner to have different ops than we do.
//! Recent gossip is also solely responsible for gossiping info about other Agents, which it does using
//! the same method of employing bloom filters.
//!
//! Historical gossip is for everything else, namely for Ops which were created more than N minutes ago.
//! It is "optimistic" in that it expects ops to be mostly the same between nodes. Rather than bloom
//! filters, it uses a novel method of splitting the possible hash space into a number of regions with
//! deterministic hashes associated with each based on the contents, which are sent to the gossip partner.
//! For regions which mismatch, the ops in those regions will be exchanged between partners. For regions
//! which match, no data will be transferred.

pub mod sharded_gossip;

mod common;
pub use common::*;
