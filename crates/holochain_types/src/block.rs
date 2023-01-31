use holo_hash::AnyDhtHash;
use holochain_zome_types::CellId;
use holochain_zome_types::Timestamp;
use rusqlite::types::ToSqlOutput;
use rusqlite::ToSql;

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
// @todo this is probably wrong.
type IpV4 = [u8; 4];

/// Target of a block.
/// Each target type has an ID and associated reason.
#[derive(Clone)]
pub enum BlockTarget {
    /// Some cell did bad at the happ level.
    Cell(CellId, CellBlockReason),
    /// Some node is playing silly buggers.
    Node(NodeId, NodeBlockReason),
    /// An entire college campus has it out for us.
    IP(IpV4, IPBlockReason),
}

#[derive(Debug, serde::Serialize)]
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

impl ToSql for BlockTargetId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            holochain_serialized_bytes::encode(&self)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                .into(),
        ))
    }
}

#[derive(Debug, serde::Serialize)]
pub enum BlockTargetReason {
    Cell(CellBlockReason),
    Node(NodeBlockReason),
    IP(IPBlockReason),
}

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
#[derive(Clone)]
pub struct Block {
    /// Target of the block.
    pub target: BlockTarget,
    /// Start time of the block. None = forever in the past.
    pub start: Option<Timestamp>,
    /// End time of the block. None = forever in the future.
    pub end: Option<Timestamp>,
}