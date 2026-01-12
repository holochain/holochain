// Temporarily allowing deprecation because of [`BlockTarget::NodeDna`] and [`BlockTarget::Node`].
#![allow(deprecated)]

use crate::prelude::*;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holochain_integrity_types::Timestamp;
use holochain_timestamp::InclusiveTimestampInterval;
#[cfg(feature = "rusqlite")]
use rusqlite::types::FromSql;
#[cfg(feature = "rusqlite")]
use rusqlite::types::ToSqlOutput;
#[cfg(feature = "rusqlite")]
use rusqlite::ToSql;

/// Reason why we might want to block a cell.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq)]
pub enum CellBlockReason {
    /// We don't know the reason but the happ does.
    #[serde(with = "serde_bytes")]
    App(Vec<u8>),
    /// Invalid validation result.
    InvalidOp(DhtOpHash),
    /// Some bad cryptography.
    BadCrypto,
}

/// Reason why we might want to block a node.
#[deprecated(since = "0.6.0")]
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum NodeBlockReason {
    /// The node did some bad cryptography.
    BadCrypto,
    /// Dos attack.
    DoS,
}

/// Reason for a Node/Space Block.
#[deprecated(since = "0.6.0")]
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq, Hash)]
pub enum NodeSpaceBlockReason {
    /// Bad message encoding.
    BadWire,
}

/// Reason why we might want to block an IP.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum IpBlockReason {
    /// Classic DoS.
    DoS,
}

/// The type to use for identifying blocking ipv4 addresses.
type IpV4 = std::net::Ipv4Addr;

/// An ID to identify a Node by.
#[deprecated(since = "0.6.0")]
pub type NodeId = String;

/// Target of a block.
/// Each target type has an ID and associated reason.
#[derive(Clone, Debug)]
pub enum BlockTarget {
    /// Block an agent for a DNA, encoded in a cell ID.
    Cell(CellId, CellBlockReason),
    #[deprecated(since = "0.6.0", note = "not respected, use cell instead")]
    NodeDna(NodeId, DnaHash, NodeSpaceBlockReason),
    /// Some node is playing silly buggers.
    #[deprecated(since = "0.6.0", note = "not respected")]
    Node(NodeId, NodeBlockReason),
    /// Currently not supported
    Ip(IpV4, IpBlockReason),
}

#[derive(
    Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, SerializedBytes, Clone,
)]
pub enum BlockTargetId {
    Cell(CellId),
    #[deprecated(since = "0.6.0", note = "not respected, use cell instead")]
    NodeDna(NodeId, DnaHash),
    #[deprecated(since = "0.6.0", note = "not respected")]
    Node(NodeId),
    Ip(IpV4),
}

impl From<BlockTarget> for BlockTargetId {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(id, _) => Self::Cell(id),
            BlockTarget::NodeDna(node_id, dna, _) => Self::NodeDna(node_id, dna),
            BlockTarget::Node(id, _) => Self::Node(id),
            BlockTarget::Ip(id, _) => Self::Ip(id),
        }
    }
}

#[cfg(feature = "rusqlite")]
impl ToSql for BlockTargetId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            holochain_serialized_bytes::encode(&self)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                .into(),
        ))
    }
}

#[cfg(feature = "rusqlite")]
impl FromSql for BlockTargetId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let bytes = match value {
            rusqlite::types::ValueRef::Blob(b) => b,
            _ => {
                // Anything else is a type‑mismatch.
                return Err(rusqlite::types::FromSqlError::InvalidType);
            }
        };

        // Decode the byte slice back into a [`BlockTargetId`].
        holochain_serialized_bytes::decode::<_, BlockTargetId>(bytes).map_err(|_| {
            // Propagate the decoding error as an invalid type error.
            rusqlite::types::FromSqlError::InvalidType
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum BlockTargetReason {
    Cell(CellBlockReason),
    #[deprecated(since = "0.6.0", note = "not respected, use cell instead")]
    NodeDna(NodeSpaceBlockReason),
    #[deprecated(since = "0.6.0", note = "not respected")]
    Node(NodeBlockReason),
    Ip(IpBlockReason),
}

#[cfg(feature = "rusqlite")]
impl ToSql for BlockTargetReason {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            holochain_serialized_bytes::encode(&self)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                .into(),
        ))
    }
}

#[cfg(feature = "rusqlite")]
impl FromSql for BlockTargetReason {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let bytes = match value {
            rusqlite::types::ValueRef::Blob(b) => b,
            _ => {
                // Anything else is a type‑mismatch.
                return Err(rusqlite::types::FromSqlError::InvalidType);
            }
        };

        // Decode the byte slice back into a [`BlockTargetReason`].
        holochain_serialized_bytes::decode::<_, BlockTargetReason>(bytes).map_err(|_| {
            // Propagate the decoding error as an invalid type error.
            rusqlite::types::FromSqlError::InvalidType
        })
    }
}

impl From<BlockTarget> for BlockTargetReason {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(_, reason) => BlockTargetReason::Cell(reason),
            BlockTarget::NodeDna(_, _, reason) => BlockTargetReason::NodeDna(reason),
            BlockTarget::Node(_, reason) => BlockTargetReason::Node(reason),
            BlockTarget::Ip(_, reason) => BlockTargetReason::Ip(reason),
        }
    }
}

/// Represents a block.
/// Also can represent an unblock.
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
