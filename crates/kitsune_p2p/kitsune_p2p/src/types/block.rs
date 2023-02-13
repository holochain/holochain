use std::sync::Arc;

enum AgentSpaceBlockReason {
    BadCrypto
}

enum NodeBlockReason {
    /// The node did some bad cryptography.
    BadCrypto,
    /// DOS attack.
    DOS,
}

enum IPBlockReason {
    /// Classic DOS.
    DOS,
}

enum BlockTarget {
    AgentSpace(Arc<crate::KitsuneAgent>, Arc<crate::KitsuneSpace>, AgentSpaceBlockReason),
    Node(Arc<[u8; 32]>, NodeBlockReason),
    Ip(std::net::Ipv4Addr, IPBlockReason),
}

pub struct Block {
    target: BlockTarget,
    start: kitsune_p2p_timestamp::Timestamp,
    end: kitsune_p2p_timestamp::Timestamp,
}