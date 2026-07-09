// Temporarily allowing deprecation because of [`BlockTarget::NodeDna`] and [`BlockTarget::Node`].
#![allow(deprecated)]

use crate::prelude::*;
use holo_hash::DhtOpHash;
use holochain_integrity_types::Timestamp;
use holochain_timestamp::InclusiveTimestampInterval;

/// Reason why we might want to block a cell.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq)]
pub enum CellBlockReason {
    /// Invalid validation result.
    InvalidOp(DhtOpHash),
    /// Some bad cryptography.
    BadCrypto,
}

/// Reason why we might want to block an IP.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum IpBlockReason {
    /// Classic DoS.
    DoS,
}

/// The type to use for identifying blocking ipv4 addresses.
type IpV4 = std::net::Ipv4Addr;

/// Target of a block.
/// Each target type has an ID and associated reason.
#[derive(Clone, Debug)]
pub enum BlockTarget {
    /// Block an agent for a DNA, encoded in a cell ID.
    Cell(CellId, CellBlockReason),
    /// Currently not supported
    Ip(IpV4, IpBlockReason),
}

#[derive(
    Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, SerializedBytes, Clone,
)]
pub enum BlockTargetId {
    Cell(CellId),
    Ip(IpV4),
}

impl From<BlockTarget> for BlockTargetId {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(id, _) => Self::Cell(id),
            BlockTarget::Ip(id, _) => Self::Ip(id),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum BlockTargetReason {
    Cell(CellBlockReason),
    Ip(IpBlockReason),
}

impl From<BlockTarget> for BlockTargetReason {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(_, reason) => BlockTargetReason::Cell(reason),
            BlockTarget::Ip(_, reason) => BlockTargetReason::Ip(reason),
        }
    }
}

/// Represents a block.
/// NOT serializable and NOT pub fields by design. `try_new` MUST be the only
/// entrypoint to build a `Block` as this enforces that the start/end times are
/// valid according to invariants the SQL queries rely on to avoid corrupting the
/// database.
#[derive(Clone, Debug)]
pub struct Block {
    /// Target of the block.
    target: BlockTarget,
    interval: InclusiveTimestampInterval,
}

impl Block {
    pub fn new(target: BlockTarget, interval: InclusiveTimestampInterval) -> Self {
        Self { target, interval }
    }

    pub fn target(&self) -> &BlockTarget {
        &self.target
    }

    pub fn interval(&self) -> &InclusiveTimestampInterval {
        &self.interval
    }

    pub fn start(&self) -> Timestamp {
        self.interval.start()
    }

    pub fn end(&self) -> Timestamp {
        self.interval.end()
    }
}
