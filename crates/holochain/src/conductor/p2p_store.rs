//! A simple KvBuf for AgentInfoSigned.

use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_state::buffer::KvStore;
use holochain_state::db::GetDb;
use holochain_state::env::EnvironmentRead;
use holochain_state::error::DatabaseResult;
use holochain_state::key::BufKey;

const AGENT_KEY_LEN: usize = 64;
const AGENT_KEY_COMPONENT_LEN: usize = 32;

/// Required new type for KvBuf key.
pub struct AgentKvKey([u8; AGENT_KEY_LEN]);

impl PartialEq for AgentKvKey {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Eq for AgentKvKey {}

impl PartialOrd for AgentKvKey {
    fn partial_cmp(&self, other: &AgentKvKey) -> Option<std::cmp::Ordering> {
        PartialOrd::partial_cmp(&&self.0[..], &&other.0[..])
    }
}

impl Ord for AgentKvKey {
    fn cmp(&self, other: &AgentKvKey) -> std::cmp::Ordering {
        Ord::cmp(&&self.0[..], &&other.0[..])
    }
}

impl From<&AgentInfoSigned> for AgentKvKey {
    fn from(o: &AgentInfoSigned) -> Self {
        (
            o.as_agent_info_ref().as_space_ref(),
            o.as_agent_info_ref().as_agent_ref(),
        )
            .into()
    }
}

impl From<(&kitsune_p2p::KitsuneSpace, &kitsune_p2p::KitsuneAgent)> for AgentKvKey {
    fn from(o: (&kitsune_p2p::KitsuneSpace, &kitsune_p2p::KitsuneAgent)) -> Self {
        use kitsune_p2p::KitsuneBinType;
        let mut bytes = [0; AGENT_KEY_LEN];
        bytes[..AGENT_KEY_COMPONENT_LEN].copy_from_slice(&o.0.get_bytes());
        bytes[AGENT_KEY_COMPONENT_LEN..].copy_from_slice(&o.1.get_bytes());
        Self(bytes)
    }
}

impl AsRef<[u8]> for AgentKvKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl BufKey for AgentKvKey {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        assert_eq!(
            bytes.len(),
            AGENT_KEY_LEN,
            "AgentKvKey needs to be {} bytes long, found {} bytes",
            AGENT_KEY_LEN,
            bytes.len()
        );
        let mut inner = [0; AGENT_KEY_LEN];
        inner.copy_from_slice(bytes);
        Self(inner)
    }
}

/// Defines the structure of the KvBuf for AgentInfoSigned.
pub struct AgentKv(KvStore<AgentKvKey, AgentInfoSigned>);

impl AsRef<KvStore<AgentKvKey, AgentInfoSigned>> for AgentKv {
    fn as_ref(&self) -> &KvStore<AgentKvKey, AgentInfoSigned> {
        &self.0
    }
}

impl AgentKv {
    /// Thin AsRef wrapper for the inner store.
    pub fn as_store_ref(&self) -> &KvStore<AgentKvKey, AgentInfoSigned> {
        self.as_ref()
    }
}

impl AgentKv {
    /// Constructor.
    pub fn new(env: EnvironmentRead) -> DatabaseResult<Self> {
        let db = env.get_db(&*holochain_state::db::AGENT)?;
        Ok(Self(KvStore::new(db)))
    }
}

#[cfg(test)]
mod tests {

    use super::AgentKvKey;
    use fixt::prelude::*;
    use holochain_p2p::kitsune_p2p::fixt::AgentInfoSignedFixturator;
    use holochain_state::buffer::KvStoreT;
    use holochain_state::env::ReadManager;
    use holochain_state::env::WriteManager;
    use holochain_state::test_utils::test_p2p_env;
    use kitsune_p2p::KitsuneBinType;

    #[test]
    fn kv_key_from() {
        let agent_info_signed = fixt!(AgentInfoSigned);

        let kv_key = AgentKvKey::from(&agent_info_signed);

        let bytes = kv_key.as_ref().to_owned();

        assert_eq!(
            &bytes[..32],
            agent_info_signed
                .as_agent_info_ref()
                .as_space_ref()
                .get_bytes(),
        );

        assert_eq!(
            &bytes[32..],
            agent_info_signed
                .as_agent_info_ref()
                .as_agent_ref()
                .get_bytes(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_store_agent_info_signed() {
        holochain_types::observability::test_run().ok();

        let test_env = test_p2p_env();
        let environ = test_env.env();

        let store_buf = super::AgentKv::new(environ.clone().into()).unwrap();

        let agent_info_signed = fixt!(AgentInfoSigned);

        let env = environ.guard();
        env.with_commit(|writer| {
            store_buf
                .as_store_ref()
                .put(writer, &(&agent_info_signed).into(), &agent_info_signed)
        })
        .unwrap();

        let ret = &store_buf
            .as_store_ref()
            .get(&env.reader().unwrap(), &(&agent_info_signed).into())
            .unwrap();

        assert_eq!(ret, &Some(agent_info_signed),);
    }
}
