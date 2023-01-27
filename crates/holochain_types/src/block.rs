use holochain_zome_types::Timestamp;
use holochain_zome_types::CellId;
use holo_hash::AnyDhtHash;

/// Reason why we might want to block a cell.
#[derive(serde::Serialize, serde::Deserialize)]
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
pub enum NodeBlockReason {
    /// The node did some bad cryptography.
    BadCrypto,
    /// DOS attack.
    DOS,
}

/// Reason why we might want to block an IP.
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
pub enum BlockTarget {
    /// Some cell did bad at the happ level.
    Cell(CellId, CellBlockReason),
    /// Some node is playing silly buggers.
    Node(NodeId, NodeBlockReason),
    /// An entire college campus has it out for us.
    IP(IpV4, IPBlockReason),
}

/// Represents a block.
/// Also can represent an unblock.
pub struct Block {
    /// Target of the block.
    pub target: BlockTarget,
    /// Start time of the block. None = forever in the past.
    pub start: Option<Timestamp>,
    /// End time of the block. None = forever in the future.
    pub end: Option<Timestamp>,
}