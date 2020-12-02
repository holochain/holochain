//! Simple methods for generating collections of AgentPubKeys for use in tests

use futures::StreamExt;
use holo_hash::AgentPubKey;
use holochain_keystore::KeystoreSender;

/// Provides simple methods for generating collections of AgentPubKeys for use in tests
pub struct TestAgents;

impl TestAgents {
    /// Get an infinite stream of AgentPubKeys
    pub fn stream(keystore: KeystoreSender) -> impl futures::Stream<Item = AgentPubKey> {
        use holochain_keystore::KeystoreSenderExt;
        futures::stream::unfold(keystore, |keystore| async {
            let key = keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .expect("can generate AgentPubKey");
            Some((key, keystore))
        })
    }

    /// Get one AgentPubKey
    pub async fn one(keystore: KeystoreSender) -> AgentPubKey {
        let mut agents: Vec<AgentPubKey> = Self::stream(keystore).take(1).collect().await;
        agents.pop().unwrap()
    }

    /// Get two AgentPubKeys
    pub async fn two(keystore: KeystoreSender) -> (AgentPubKey, AgentPubKey) {
        let mut agents: Vec<AgentPubKey> = Self::stream(keystore).take(2).collect().await;
        (agents.pop().unwrap(), agents.pop().unwrap())
    }

    /// Get three AgentPubKeys
    pub async fn three(keystore: KeystoreSender) -> (AgentPubKey, AgentPubKey, AgentPubKey) {
        let mut agents: Vec<_> = Self::stream(keystore).take(3).collect().await;
        (
            agents.pop().unwrap(),
            agents.pop().unwrap(),
            agents.pop().unwrap(),
        )
    }
}
