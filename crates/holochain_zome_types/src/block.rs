use crate::CellId;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
<<<<<<< HEAD
use holo_hash::DnaHash;
use holochain_integrity_types::Timestamp;
use kitsune_p2p_timestamp::InclusiveTimestampInterval;
=======
use holochain_integrity_types::Timestamp;
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
#[cfg(feature = "rusqlite")]
use rusqlite::types::ToSqlOutput;
#[cfg(feature = "rusqlite")]
use rusqlite::ToSql;
<<<<<<< HEAD
=======
use thiserror::Error;
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa

// Everything required for a coordinator to block some agent on the same DNA.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BlockAgentInput {
    pub target: AgentPubKey,
    // Reason is literally whatever you want it to be.
    // But unblock must be an exact match.
    #[serde(with = "serde_bytes")]
    pub reason: Vec<u8>,
<<<<<<< HEAD
    pub interval: InclusiveTimestampInterval,
=======
    pub start: Timestamp,
    pub end: Timestamp,
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
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

<<<<<<< HEAD
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
=======
/// Reason why we might want to block a node.
#[derive(Clone, serde::Serialize, Debug)]
pub enum NodeBlockReason {
    /// The node did some bad cryptography.
    BadCrypto,
    /// DOS attack.
    DOS,
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
}

/// Reason why we might want to block an IP.
#[derive(Clone, serde::Serialize, Debug)]
<<<<<<< HEAD
pub enum IpBlockReason {
    Kitsune(kitsune_p2p_block::IpBlockReason),
}

impl From<kitsune_p2p_block::IpBlockReason> for IpBlockReason {
    fn from(kitsune_ip_block_reason: kitsune_p2p_block::IpBlockReason) -> Self {
        Self::Kitsune(kitsune_ip_block_reason)
    }
}
=======
pub enum IPBlockReason {
    /// Classic DOS.
    DOS,
}

// @todo this is probably wrong.
type NodeId = [u8; 32];
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa

/// The type to use for identifying blocking ipv4 addresses.
type IpV4 = std::net::Ipv4Addr;

/// Target of a block.
/// Each target type has an ID and associated reason.
#[derive(Clone, Debug)]
pub enum BlockTarget {
    /// Some cell did bad at the happ level.
    Cell(CellId, CellBlockReason),
    /// Some node is playing silly buggers.
<<<<<<< HEAD
    Node(kitsune_p2p_block::NodeId, NodeBlockReason),
    /// An entire college campus has it out for us.
    Ip(IpV4, IpBlockReason),
}

impl From<kitsune_p2p_block::BlockTarget> for BlockTarget {
    fn from(kblock_target: kitsune_p2p_block::BlockTarget) -> Self {
        match kblock_target {
            kitsune_p2p_block::BlockTarget::AgentSpace(agent, space, reason) => Self::Cell(
                CellId::new(
                    DnaHash::from_raw_36(space.0.clone()),
                    AgentPubKey::from_raw_36(agent.0.clone()),
                ),
                reason.into(),
            ),
            kitsune_p2p_block::BlockTarget::Node(node_id, reason) => {
                Self::Node(node_id, reason.into())
            }
            kitsune_p2p_block::BlockTarget::Ip(ip_addr, reason) => Self::Ip(ip_addr, reason.into()),
        }
    }
=======
    Node(NodeId, NodeBlockReason),
    /// An entire college campus has it out for us.
    IP(IpV4, IPBlockReason),
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
}

#[derive(Debug, serde::Serialize, Clone)]
pub enum BlockTargetId {
    Cell(CellId),
<<<<<<< HEAD
    Node(kitsune_p2p_block::NodeId),
    Ip(IpV4),
}

impl From<kitsune_p2p_block::BlockTargetId> for BlockTargetId {
    fn from(kblock_target_id: kitsune_p2p_block::BlockTargetId) -> Self {
        match kblock_target_id {
            kitsune_p2p_block::BlockTargetId::AgentSpace(agent, space) => Self::Cell(CellId::new(
                DnaHash::from_raw_36(space.0.clone()),
                AgentPubKey::from_raw_36(agent.0.clone()),
            )),
            kitsune_p2p_block::BlockTargetId::Node(node_id) => Self::Node(node_id),
            kitsune_p2p_block::BlockTargetId::Ip(ip_addr) => Self::Ip(ip_addr),
        }
    }
=======
    Node(NodeId),
    IP(IpV4),
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
}

impl From<BlockTarget> for BlockTargetId {
    fn from(block_target: BlockTarget) -> Self {
        match block_target {
            BlockTarget::Cell(id, _) => Self::Cell(id),
            BlockTarget::Node(id, _) => Self::Node(id),
<<<<<<< HEAD
            BlockTarget::Ip(id, _) => Self::Ip(id),
=======
            BlockTarget::IP(id, _) => Self::IP(id),
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
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
<<<<<<< HEAD
    Ip(IpBlockReason),
=======
    IP(IPBlockReason),
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
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
<<<<<<< HEAD
            BlockTarget::Ip(_, reason) => BlockTargetReason::Ip(reason),
=======
            BlockTarget::IP(_, reason) => BlockTargetReason::IP(reason),
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
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
<<<<<<< HEAD
    interval: InclusiveTimestampInterval,
}

impl From<kitsune_p2p_block::Block> for Block {
    fn from(kblock: kitsune_p2p_block::Block) -> Self {
        Self {
            target: kblock.clone().into_target().into(),
            interval: kblock.into_interval(),
        }
=======
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
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
    }
}

impl Block {
<<<<<<< HEAD
    pub fn new(target: BlockTarget, interval: InclusiveTimestampInterval) -> Self {
        Self { target, interval }
=======
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
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
    }

    pub fn target(&self) -> &BlockTarget {
        &self.target
    }

<<<<<<< HEAD
    pub fn interval(&self) -> &InclusiveTimestampInterval {
        &self.interval
    }

    pub fn start(&self) -> &Timestamp {
        self.interval.start()
    }

    pub fn end(&self) -> &Timestamp {
        self.interval.end()
=======
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
>>>>>>> d8fbfccb2aadae0ac89943c0b5be653d5f7916aa
    }
}
