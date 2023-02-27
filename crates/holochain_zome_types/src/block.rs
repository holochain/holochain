use crate::CellId;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holochain_integrity_types::Timestamp;
#[cfg(feature = "rusqlite")]
use rusqlite::types::ToSqlOutput;
#[cfg(feature = "rusqlite")]
use rusqlite::ToSql;
use thiserror::Error;

// Everything required for a coordinator to block some agent on the same DNA.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BlockAgentInput {
    pub target: AgentPubKey,
    // Reason is literally whatever you want it to be.
    // But unblock must be an exact match.
    #[serde(with = "serde_bytes")]
    pub reason: Vec<u8>,
    pub start: Timestamp,
    pub end: Timestamp,
}

/// Reason why we might want to block a cell.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum CellBlockReason {
    /// We don't know the reason but the happ does.
    #[serde(with = "serde_bytes")]
    App(Vec<u8>),
    /// Invalid validation result.
    InvalidOp(AnyDhtHash),
    /// Some bad cryptography.
    BadCrypto,
}

/// Reason why we might want to block a node.
#[derive(Clone, serde::Serialize, Debug)]
pub enum NodeBlockReason {
    /// The node did some bad cryptography.
    BadCrypto,
    /// DOS attack.
    DOS,
}

/// Reason why we might want to block an IP.
#[derive(Clone, serde::Serialize, Debug)]
pub enum IPBlockReason {
    /// Classic DOS.
    DOS,
}

// @todo this is probably wrong.
type NodeId = [u8; 32];

/// The type to use for identifying blocking ipv4 addresses.
type IpV4 = std::net::Ipv4Addr;

/// Target of a block.
/// Each target type has an ID and associated reason.
#[derive(Clone, Debug)]
pub enum BlockTarget {
    /// Some cell did bad at the happ level.
    Cell(CellId, CellBlockReason),
    /// Some node is playing silly buggers.
    Node(NodeId, NodeBlockReason),
    /// An entire college campus has it out for us.
    IP(IpV4, IPBlockReason),
}

#[derive(Debug, serde::Serialize, Clone)]
pub enum BlockTargetId {
    Cell(CellId),
    Node(NodeId),
    IP(IpV4),
}

impl From<BlockTarget> for BlockTargetId {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(id, _) => Self::Cell(id),
            BlockTarget::Node(id, _) => Self::Node(id),
            BlockTarget::IP(id, _) => Self::IP(id),
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

#[derive(Debug, serde::Serialize, Clone)]
pub enum BlockTargetReason {
    Cell(CellBlockReason),
    Node(NodeBlockReason),
    IP(IPBlockReason),
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

impl From<BlockTarget> for BlockTargetReason {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(_, reason) => BlockTargetReason::Cell(reason),
            BlockTarget::Node(_, reason) => BlockTargetReason::Node(reason),
            BlockTarget::IP(_, reason) => BlockTargetReason::IP(reason),
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
    /// Start time of the block. None = forever in the past.
    start: Timestamp,
    /// End time of the block. None = forever in the future.
    end: Timestamp,
}

#[derive(Debug, Error)]
pub enum BlockError {
    InvalidTimes(Timestamp, Timestamp),
}

impl std::fmt::Display for BlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Block {
    pub fn try_new(
        target: BlockTarget,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<Self, BlockError> {
        if start > end {
            Err(BlockError::InvalidTimes(start, end))
        } else {
            Ok(Self { target, start, end })
        }
    }

    pub fn target(&self) -> &BlockTarget {
        &self.target
    }

    pub fn start(&self) -> Timestamp {
        self.start
    }

    pub fn end(&self) -> Timestamp {
        self.end
    }
}

#[cfg(test)]
mod test {
    use super::BlockTarget;
    use super::CellBlockReason;
    use crate::CellIdFixturator;
    use holochain_integrity_types::Timestamp;

    #[test]
    fn block_test_new() {
        let target = BlockTarget::Cell(fixt::fixt!(CellId), CellBlockReason::BadCrypto);

        // valids.
        for (start, end) in vec![(0, 0), (-1, 0), (0, 1), (i64::MIN, i64::MAX)] {
            super::Block::try_new(target.clone(), Timestamp(start), Timestamp(end)).unwrap();
        }

        // invalids.
        for (start, end) in vec![(0, -1), (1, 0), (i64::MAX, i64::MIN)] {
            assert!(
                super::Block::try_new(target.clone(), Timestamp(start), Timestamp(end)).is_err()
            );
        }
    }
}
