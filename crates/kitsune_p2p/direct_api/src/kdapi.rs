//! kdirect kdapi types

use crate::*;

/// KdApi websocket communication serialization type.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum KdApi {
    /// A structured user-defined message
    #[serde(rename = "user")]
    User {
        /// The structured user-data
        user: serde_json::Value,
    },

    /// Indicates an error occurred during a request
    #[serde(rename = "errorRes")]
    ErrorRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// reason
        #[serde(rename = "reason")]
        reason: String,
    },

    /// Hello message sent from server to client on new connection
    #[serde(rename = "helloReq")]
    HelloReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// salt for passphrase hashing
        #[serde(rename = "salt")]
        salt: kd_entry::KdEntryBinary,
    },

    /// Hello response containing authentication data
    #[serde(rename = "helloRes")]
    HelloRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// argon2id hash of salt + passphrase
        #[serde(rename = "auth")]
        auth: kd_entry::KdEntryBinary,
    },

    /// If there is already a keypair associated with this tag,
    /// fetch the pubKey, otherwise generate a new pair, and return the pubKey.
    #[serde(rename = "keypairGetOrCreateTaggedReq")]
    KeypairGetOrCreateTaggedReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// unique tag associated with this keypair
        #[serde(rename = "tag")]
        tag: String,
    },

    /// Returns the pubkey of the newly created keypair
    #[serde(rename = "keypairGetOrCreateTaggedRes")]
    KeypairGetOrCreateTaggedRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// pubkey
        #[serde(rename = "pubKey")]
        pub_key: KdHash,
    },

    /// Join an agent to an app root hash
    #[serde(rename = "appJoinReq")]
    AppJoinReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the agent/author to join
        #[serde(rename = "agent")]
        agent: KdHash,
    },

    /// Success result of joining
    #[serde(rename = "appJoinRes")]
    AppJoinRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,
    },

    /// Remove an agent from an app root hash
    #[serde(rename = "appLeaveReq")]
    AppLeaveReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the agent/author to join
        #[serde(rename = "agent")]
        agent: KdHash,
    },

    /// Success result of leaving
    #[serde(rename = "appLeaveRes")]
    AppLeaveRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,
    },

    /// Inject an agent info record into the store from an outside source
    #[serde(rename = "agentInfoStoreReq")]
    AgentInfoStoreReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// agent info
        #[serde(rename = "agentInfo")]
        agent_info: KdAgentInfo,
    },

    /// Success injecting an agent info record into the store from an outside source
    #[serde(rename = "agentInfoStoreRes")]
    AgentInfoStoreRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,
    },

    /// get a specific agent_info record from the store
    #[serde(rename = "agentInfoGetReq")]
    AgentInfoGetReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the agent
        #[serde(rename = "agent")]
        agent: KdHash,
    },

    /// get a specific agent_info record from the store
    #[serde(rename = "agentInfoGetRes")]
    AgentInfoGetRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// agent info
        #[serde(rename = "agentInfo")]
        agent_info: KdAgentInfo,
    },

    /// query a list of agent_info records from the store
    #[serde(rename = "agentInfoQueryReq")]
    AgentInfoQueryReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,
    },

    /// get a specific agent_info record from the store
    #[serde(rename = "agentInfoQueryRes")]
    AgentInfoQueryRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// agent info list
        #[serde(rename = "agentInfoList")]
        agent_info_list: Vec<KdAgentInfo>,
    },

    /// check if an agent is an authority for a given hash
    #[serde(rename = "isAuthorityReq")]
    IsAuthorityReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// agent hash
        #[serde(rename = "agent")]
        agent: KdHash,

        /// basis hash
        #[serde(rename = "basis")]
        basis: KdHash,
    },

    /// check if an agent is an authority for a given hash
    #[serde(rename = "isAuthorityRes")]
    IsAuthorityRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// is authority
        #[serde(rename = "isAuthority")]
        is_authority: bool,
    },

    /// Send a message to a remote app/agent
    #[serde(rename = "messageSendReq")]
    MessageSendReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the destination agent
        #[serde(rename = "toAgent")]
        to_agent: KdHash,

        /// the agent authoring this message
        #[serde(rename = "fromAgent")]
        from_agent: KdHash,

        /// the structured content for this message
        #[serde(rename = "content")]
        content: serde_json::Value,

        /// the binary data associated with this message
        #[serde(rename = "binary")]
        binary: kd_entry::KdEntryBinary,
    },

    /// Success sending a message to a remote app/agent
    #[serde(rename = "messageSendRes")]
    MessageSendRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,
    },

    /// Receive an incoming message from a remote app/agent
    #[serde(rename = "messageRecvEvt")]
    MessageRecvEvt {
        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the destination agent
        #[serde(rename = "toAgent")]
        to_agent: KdHash,

        /// the agent authoring this message
        #[serde(rename = "fromAgent")]
        from_agent: KdHash,

        /// the structured content for this message
        #[serde(rename = "content")]
        content: serde_json::Value,

        /// the binary data associated with this message
        #[serde(rename = "binary")]
        binary: kd_entry::KdEntryBinary,
    },

    /// Author / Publish a new KdEntry
    #[serde(rename = "entryAuthorReq")]
    EntryAuthorReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the author pubkey for this entry
        #[serde(rename = "author")]
        author: KdHash,

        /// the entry content for this entry
        #[serde(rename = "content")]
        content: KdEntryContent,

        /// the binary data associated with this entry
        #[serde(rename = "binary")]
        binary: kd_entry::KdEntryBinary,
    },

    /// Returns the full KdEntrySigned data of the newly
    /// Authored / Published entry
    #[serde(rename = "entryAuthorRes")]
    EntryAuthorRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// signed entry
        #[serde(rename = "entrySigned")]
        entry_signed: KdEntrySigned,
    },

    /// Get a specific entry
    #[serde(rename = "entryGetReq")]
    EntryGetReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// the agent
        #[serde(rename = "agent")]
        agent: KdHash,

        /// hash
        #[serde(rename = "hash")]
        hash: KdHash,
    },

    /// the result of the entry get
    #[serde(rename = "entryGetRes")]
    EntryGetRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// signed entry
        #[serde(rename = "entrySigned")]
        entry_signed: KdEntrySigned,
    },

    /// Get the children of a specific entry
    #[serde(rename = "entryGetChildrenReq")]
    EntryGetChildrenReq {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// root app hash
        #[serde(rename = "root")]
        root: KdHash,

        /// hash of the parent
        #[serde(rename = "parent")]
        parent: KdHash,

        /// optional kind filter
        #[serde(rename = "kind")]
        kind: Option<String>,
    },

    /// the result of the entry get children
    #[serde(rename = "entryGetChildrenRes")]
    EntryGetChildrenRes {
        /// message id
        #[serde(rename = "msgId")]
        msg_id: String,

        /// signed entry
        #[serde(rename = "entrySignedList")]
        entry_signed_list: Vec<KdEntrySigned>,
    },
}

impl std::fmt::Display for KdApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string_pretty(&self).map_err(|_| std::fmt::Error)?;
        f.write_str(&s)?;
        Ok(())
    }
}

impl std::str::FromStr for KdApi {
    type Err = KdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(KdError::other)
    }
}

impl KdApi {
    /// Reconstruct this KdApi from a `to_string()` str.
    // this *does* implement the trait clippy...
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> KdResult<Self> {
        std::str::FromStr::from_str(s)
    }

    /// Get the msg_id (or empty string ("")) associated with this api.
    pub fn msg_id(&self) -> &str {
        match self {
            Self::User { .. } => "",
            Self::ErrorRes { msg_id, .. } => msg_id,
            Self::HelloReq { msg_id, .. } => msg_id,
            Self::HelloRes { msg_id, .. } => msg_id,
            Self::KeypairGetOrCreateTaggedReq { msg_id, .. } => msg_id,
            Self::KeypairGetOrCreateTaggedRes { msg_id, .. } => msg_id,
            Self::AppJoinReq { msg_id, .. } => msg_id,
            Self::AppJoinRes { msg_id, .. } => msg_id,
            Self::AppLeaveReq { msg_id, .. } => msg_id,
            Self::AppLeaveRes { msg_id, .. } => msg_id,
            Self::AgentInfoStoreReq { msg_id, .. } => msg_id,
            Self::AgentInfoStoreRes { msg_id, .. } => msg_id,
            Self::AgentInfoGetReq { msg_id, .. } => msg_id,
            Self::AgentInfoGetRes { msg_id, .. } => msg_id,
            Self::AgentInfoQueryReq { msg_id, .. } => msg_id,
            Self::AgentInfoQueryRes { msg_id, .. } => msg_id,
            Self::IsAuthorityReq { msg_id, .. } => msg_id,
            Self::IsAuthorityRes { msg_id, .. } => msg_id,
            Self::MessageSendReq { msg_id, .. } => msg_id,
            Self::MessageSendRes { msg_id, .. } => msg_id,
            Self::MessageRecvEvt { .. } => "",
            Self::EntryAuthorReq { msg_id, .. } => msg_id,
            Self::EntryAuthorRes { msg_id, .. } => msg_id,
            Self::EntryGetReq { msg_id, .. } => msg_id,
            Self::EntryGetRes { msg_id, .. } => msg_id,
            Self::EntryGetChildrenReq { msg_id, .. } => msg_id,
            Self::EntryGetChildrenRes { msg_id, .. } => msg_id,
        }
    }

    /// Returns true if the message is a response type.
    #[allow(clippy::match_like_matches_macro)]
    pub fn is_res(&self) -> bool {
        match self {
            Self::ErrorRes { .. } => true,
            Self::HelloRes { .. } => true,
            Self::KeypairGetOrCreateTaggedRes { .. } => true,
            Self::AppJoinRes { .. } => true,
            Self::AppLeaveRes { .. } => true,
            Self::AgentInfoStoreRes { .. } => true,
            Self::AgentInfoGetRes { .. } => true,
            Self::AgentInfoQueryRes { .. } => true,
            Self::IsAuthorityRes { .. } => true,
            Self::MessageSendRes { .. } => true,
            Self::EntryAuthorRes { .. } => true,
            Self::EntryGetRes { .. } => true,
            Self::EntryGetChildrenRes { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kd_api_encode_decode() {
        let api = KdApi::HelloReq {
            msg_id: "test".to_string(),
            salt: vec![1, 2, 3, 4].into_boxed_slice().into(),
        };
        println!("{}", api);
    }
}
