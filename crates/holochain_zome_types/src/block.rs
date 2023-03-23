use crate::CellId;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holochain_integrity_types::Timestamp;
use kitsune_p2p_block::NodeSpaceBlockReason;
use kitsune_p2p_timestamp::InclusiveTimestampInterval;
#[cfg(feature = "rusqlite")]
use rusqlite::types::ToSqlOutput;
#[cfg(feature = "rusqlite")]
use rusqlite::ToSql;

// Everything required for a coordinator to block some agent on the same DNA.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BlockAgentInput {
    pub target: AgentPubKey,
    // Reason is literally whatever you want it to be.
    // But unblock must be an exact match.
    #[serde(with = "serde_bytes")]
    pub reason: Vec<u8>,
    pub interval: InclusiveTimestampInterval,
}

/// Reason why we might want to block a cell.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum CellBlockReason {
    /// We don't know the reason but the happ does.
    #[serde(with = "serde_bytes")]
    App(Vec<u8>),
    /// Invalid validation result.
    InvalidOp(DhtOpHash),
    /// Some bad cryptography.
    BadCrypto,
}

impl From<kitsune_p2p_block::AgentSpaceBlockReason> for CellBlockReason {
    fn from(agent_space_block_reason: kitsune_p2p_block::AgentSpaceBlockReason) -> Self {
        match agent_space_block_reason {
            kitsune_p2p_block::AgentSpaceBlockReason::BadCrypto => CellBlockReason::BadCrypto,
        }
    }
}

/// Reason why we might want to block a node.
#[derive(Clone, serde::Serialize, Debug)]
pub enum NodeBlockReason {
    Kitsune(kitsune_p2p_block::NodeBlockReason),
}

impl From<kitsune_p2p_block::NodeBlockReason> for NodeBlockReason {
    fn from(kitsune_node_block_reason: kitsune_p2p_block::NodeBlockReason) -> Self {
        Self::Kitsune(kitsune_node_block_reason)
    }
}

/// Reason why we might want to block an IP.
#[derive(Clone, serde::Serialize, Debug)]
pub enum IpBlockReason {
    Kitsune(kitsune_p2p_block::IpBlockReason),
}

impl From<kitsune_p2p_block::IpBlockReason> for IpBlockReason {
    fn from(kitsune_ip_block_reason: kitsune_p2p_block::IpBlockReason) -> Self {
        Self::Kitsune(kitsune_ip_block_reason)
    }
}

/// The type to use for identifying blocking ipv4 addresses.
type IpV4 = std::net::Ipv4Addr;

/// Target of a block.
/// Each target type has an ID and associated reason.
#[derive(Clone, Debug)]
pub enum BlockTarget {
    /// Some cell did bad at the happ level.
    Cell(CellId, CellBlockReason),
    NodeDna(kitsune_p2p_block::NodeId, DnaHash, NodeSpaceBlockReason),
    /// Some node is playing silly buggers.
    Node(kitsune_p2p_block::NodeId, NodeBlockReason),
    /// An entire college campus has it out for us.
    Ip(IpV4, IpBlockReason),
}

impl From<kitsune_p2p_block::BlockTarget> for BlockTarget {
    fn from(kblock_target: kitsune_p2p_block::BlockTarget) -> Self {
        match kblock_target {
            kitsune_p2p_block::BlockTarget::NodeSpace(node_id, space, reason) => Self::NodeDna(
                node_id,
                DnaHash::from_raw_36(space.0.clone()),
                reason.into(),
            ),
            kitsune_p2p_block::BlockTarget::Node(node_id, reason) => {
                Self::Node(node_id, reason.into())
            }
            kitsune_p2p_block::BlockTarget::Ip(ip_addr, reason) => Self::Ip(ip_addr, reason.into()),
        }
    }
}

#[derive(Debug, serde::Serialize, Clone)]
pub enum BlockTargetId {
    Cell(CellId),
    NodeDna(kitsune_p2p_block::NodeId, DnaHash),
    Node(kitsune_p2p_block::NodeId),
    Ip(IpV4),
    // We don't have an ID for the remote.
    Anon,
}

impl From<kitsune_p2p_block::BlockTargetId> for BlockTargetId {
    fn from(kblock_target_id: kitsune_p2p_block::BlockTargetId) -> Self {
        match kblock_target_id {
            kitsune_p2p_block::BlockTargetId::NodeSpace(node_id, space) => {
                Self::NodeDna(node_id, DnaHash::from_raw_36(space.0.clone()))
            }
            kitsune_p2p_block::BlockTargetId::Node(node_id) => Self::Node(node_id),
            kitsune_p2p_block::BlockTargetId::Ip(ip_addr) => Self::Ip(ip_addr),
            kitsune_p2p_block::BlockTargetId::Anon => Self::Anon,
        }
    }
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

#[derive(Debug, serde::Serialize, Clone)]
pub enum BlockTargetReason {
    Cell(CellBlockReason),
    NodeDna(NodeSpaceBlockReason),
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

impl From<kitsune_p2p_block::Block> for Block {
    fn from(kblock: kitsune_p2p_block::Block) -> Self {
        Self {
            target: kblock.clone().into_target().into(),
            interval: kblock.into_interval(),
        }
    }
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
