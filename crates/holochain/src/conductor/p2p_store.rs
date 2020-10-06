//! A simple KvBuf for AgentInfoSigned.

use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_p2p::kitsune_p2p::KitsuneAgent;
use holochain_state::buffer::KvStore;
use holochain_state::db::GetDb;
use holochain_state::env::EnvironmentRead;
use holochain_state::error::DatabaseResult;
use holochain_state::key::BufKey;

/// Required new type for KvBuf key.
#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub struct AgentKvKey([0; 64])

impl From<AgentInfoSignedKey> for AgentKvKey {
    fn from(agent_info_signed_key: AgentInfoSignedKey) -> Self {
        let mut bytes = [0; 64];
        bytes[..32].copy_from_slice(agent_info_signed_key.space_bytes())
        bytes[32..].copy_from_slice(agent_info_signed_key.agent_bytes())
        Self(bytes)
    }
}

impl AsRef<[u8]> for AgentInfoSignedKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl BufKey for AgentKvKey {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self(AgentInfoSignedKey::from(bytes.to_vec()))
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

    use fixt::prelude::*;
    use holochain_p2p::kitsune_p2p::fixt::AgentInfoSignedFixturator;
    use holochain_state::buffer::KvStoreT;
    use holochain_state::env::ReadManager;
    use holochain_state::env::WriteManager;
    use holochain_state::test_utils::test_p2p_env;

    #[tokio::test(threaded_scheduler)]
    async fn test_store_agent_info_signed() {
        holochain_types::observability::test_run().ok();

        let test_env = test_p2p_env();
        let environ = test_env.env();

        let store_buf = super::AgentKv::new(environ.clone().into()).unwrap();

        let agent_info_signed = fixt!(AgentInfoSigned);

        let env = environ.guard();
        env.with_commit(|writer| {
            store_buf.as_store_ref().put(
                writer,
                &agent_info_signed
                    .as_agent_info_ref()
                    .as_id_ref()
                    .to_owned()
                    .into(),
                &agent_info_signed,
            )
        })
        .unwrap();

        let ret = &store_buf
            .as_store_ref()
            .get(
                &env.reader().unwrap(),
                &agent_info_signed
                    .as_agent_info_ref()
                    .as_id_ref()
                    .to_owned()
                    .into(),
            )
            .unwrap();

        assert_eq!(ret, &Some(agent_info_signed),);
    }
}
