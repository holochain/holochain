use kitsune_p2p_timestamp::InclusiveTimestampInterval;
use kitsune_p2p_timestamp::Timestamp;
use std::sync::Arc;

#[derive(Clone)]
pub enum AgentSpaceBlockReason {
    BadCrypto,
}

#[derive(Clone, serde::Serialize, Debug)]
pub enum NodeBlockReason {
    /// The node did some bad cryptography.
    BadCrypto,
    /// DOS attack.
    DOS,
}

#[derive(Clone, serde::Serialize, Debug)]
pub enum IpBlockReason {
    /// Classic DOS.
    DOS,
}

pub type NodeId = Arc<[u8; 32]>;

#[derive(Clone)]
pub enum BlockTarget {
    AgentSpace(
        Arc<kitsune_p2p_types::bin_types::KitsuneAgent>,
        Arc<kitsune_p2p_types::bin_types::KitsuneSpace>,
        AgentSpaceBlockReason,
    ),
    Node(NodeId, NodeBlockReason),
    Ip(std::net::Ipv4Addr, IpBlockReason),
}

#[derive(Clone)]
pub struct Block {
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

    pub fn into_target(self) -> BlockTarget {
        self.target
    }

    pub fn into_interval(self) -> InclusiveTimestampInterval {
        self.interval
    }

    pub fn start(&self) -> &Timestamp {
        &self.interval.start()
    }

    pub fn end(&self) -> &Timestamp {
        &self.interval.end()
    }
}
