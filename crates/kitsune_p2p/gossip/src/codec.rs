use std::sync::Arc;

use kitsune_p2p_dht::region_set::RegionSetLtcs;
use kitsune_p2p_fetch::OpHashSized;
use kitsune_p2p_types::{agent_info::AgentInfoSigned, dht_arc::DhtArcRange, KAgent};

use crate::{
    bloom::{EncodedBloom, EncodedTimedBloomFilter},
    round::GossipPlan,
};

kitsune_p2p_types::write_codec_enum! {
    /// ShardedGossip Wire Protocol Codec
    codec GossipMsg {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            local_arcset.0: Vec<DhtArcRange>,

            /// A random number to resolve concurrent initiates.
            tiebreaker.1: u32,

            /// List of active local agents represented by this node.
            local_agents.2: Vec<KAgent>,

            /// The plan for this gossip round
            plan.3: GossipPlan,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            local_arcset.0: Vec<DhtArcRange>,

            /// List of active local agents represented by this node.
            local_agents.1: Vec<KAgent>,

            /// The plan for this gossip round
            plan.2: GossipPlan,
        },

        /// Send Agent Info Bloom
        AgentDiff(0x30) {
            /// The bloom filter for agent data
            bloom_filter.0: EncodedBloom,
        },

        /// Any agents that were missing from the remote bloom.
        AgentData(0x40) {
            /// The missing agents
            agents.0: Vec<Arc<AgentInfoSigned>>,
        },

        /// Send Op Bloom filter
        OpBloom(0x50) {
            /// The bloom filter for op data
            bloom_filter.0: EncodedTimedBloomFilter,
            /// Is this the last bloom to be sent?
            finished.1: bool,
        },

        /// Send Op regions
        OpRegions(0x51) {
            /// The region set
            regions.0: RegionSetLtcs,
        },

        /// Any ops that were missing from the remote bloom.
        OpData(0x60) {
            /// The missing op hashes
            ops.0: Vec<OpHashSized>,
            /// Ops that are missing from a bloom that you have sent.
            /// These will be chunked into a maximum size of about 16MB.
            /// If the amount of missing ops is larger then the
            /// [`ShardedGossipLocal::UPPER_BATCH_BOUND`] then the set of
            /// missing ops chunks will be sent in batches.
            /// Each batch will require a reply message of [`OpBatchReceived`]
            /// in order to get the next batch.
            /// This is to prevent overloading the receiver with too much
            /// incoming data.
            ///
            /// 0: There is more chunks in this batch to come. No reply is needed.
            /// 1: This chunk is done but there is more batches
            /// to come and you should reply with [`OpBatchReceived`]
            /// when you are ready to get the next batch.
            /// 2: This is the final missing ops and there
            /// are no more ops to come. No reply is needed.
            ///
            /// See [`MissingOpsStatus`]
            finished.1: u8,
        },

        /// I have received a complete batch of
        /// missing ops and I am ready to receive the
        /// next batch.
        OpBatchReceived(0x61) {
        },


        /// The node you are gossiping with has hit an error condition
        /// and failed to respond to a request.
        Error(0xa0) {
            /// The error message.
            message.0: String,
        },

        /// The node currently is gossiping with too many
        /// other nodes and is too busy to accept your initiate.
        /// Please try again later.
        Busy(0xa1) {
        },

        /// The node you are trying to gossip with has no agents anymore.
        NoAgents(0xa2) {
        },

        /// You have sent a stale initiate to a node
        /// that already has an active round with you.
        AlreadyInProgress(0xa3) {
        },

        /// You have sent a stale initiate to a node
        /// that already has an active round with you.
        UnexpectedMessage(0xaf) {
        },
    }
}
