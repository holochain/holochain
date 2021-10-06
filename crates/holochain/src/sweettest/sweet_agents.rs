//! Simple methods for generating collections of AgentPubKeys for use in tests

use futures::StreamExt;
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;

/// Provides simple methods for generating collections of AgentPubKeys for use in tests
pub struct SweetAgents;

impl SweetAgents {
    /// Get an infinite stream of AgentPubKeys
    pub fn stream(keystore: MetaLairClient) -> impl futures::Stream<Item = AgentPubKey> {
        use holochain_keystore::KeystoreSenderExt;
        futures::stream::unfold(keystore, |keystore| async {
            let key = keystore
                .unwrap_legacy()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .expect("can generate AgentPubKey");
            Some((key, keystore))
        })
    }

    /// Get a Vec of AgentPubKeys
    pub async fn get(keystore: MetaLairClient, num: usize) -> Vec<AgentPubKey> {
        Self::stream(keystore).take(num).collect().await
    }

    /// Get one AgentPubKey
    pub async fn one(keystore: MetaLairClient) -> AgentPubKey {
        let mut agents = Self::get(keystore, 1).await;
        agents.pop().unwrap()
    }

    /// Get two AgentPubKeys
    pub async fn two(keystore: MetaLairClient) -> (AgentPubKey, AgentPubKey) {
        let mut agents = Self::get(keystore, 2).await;
        (agents.pop().unwrap(), agents.pop().unwrap())
    }

    /// Get three AgentPubKeys
    pub async fn three(keystore: MetaLairClient) -> (AgentPubKey, AgentPubKey, AgentPubKey) {
        let mut agents = Self::get(keystore, 3).await;
        (
            agents.pop().unwrap(),
            agents.pop().unwrap(),
            agents.pop().unwrap(),
        )
    }
}
