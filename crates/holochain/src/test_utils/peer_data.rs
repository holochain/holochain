//! Utility to inject [AgentInfo] to the peer database
//! for tests where we don't want to use a bootstrap server.

use std::{
    convert::TryInto,
    time::{Duration, UNIX_EPOCH},
};

use crate::conductor::p2p_store::AgentKv;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_keystore::{AgentPubKeyExt, KeystoreSender};
use holochain_state::{
    buffer::KvStoreT, env::EnvironmentWrite, env::WriteManager, error::DatabaseResult,
};
use holochain_zome_types::signature::Signature;
use kitsune_p2p::{
    agent_store::{AgentInfo, AgentInfoSigned, Urls},
    KitsuneSignature,
};

use self::error::PeerDataResult;

/// Inject peer info into the peer store for testing
pub fn inject_peer_data(
    env: EnvironmentWrite,
    agent_info_signed: AgentInfoSigned,
) -> DatabaseResult<()> {
    let p2p_kv = AgentKv::new(env.clone().into())?;
    let env_ref = env.guard();
    Ok(env_ref.with_commit(|writer| {
        p2p_kv.as_store_ref().put(
            writer,
            &(&agent_info_signed).try_into()?,
            &agent_info_signed,
        )
    })?)
}

/// Create a signed agent info with a ttl.
/// If ttl is none it will default to 1 day.
pub async fn create_signed_agent_info(
    agent: AgentPubKey,
    space: DnaHash,
    urls: Urls,
    ttl: Option<Duration>,
    keystore: &KeystoreSender,
) -> PeerDataResult<AgentInfoSigned> {
    use holochain_p2p::AgentPubKeyExt;
    let signed_at_ms = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let expires_after_ms = ttl
        .unwrap_or_else(|| Duration::from_secs(60 * 60 * 24))
        .as_millis()
        .try_into()?;
    let info = to_agent_info(
        agent.clone(),
        space.clone(),
        urls,
        signed_at_ms,
        expires_after_ms,
    );
    let info_bytes = agent_info_to_bytes(&info).await?;
    let signature = sign_agent_info(&agent, &info_bytes[..], keystore).await?;
    Ok(AgentInfoSigned::try_new(
        agent.into_kitsune_raw(),
        KitsuneSignature::from(signature.0),
        info_bytes,
    )?)
}

fn to_agent_info(
    agent: AgentPubKey,
    space: DnaHash,
    urls: Urls,
    signed_at_ms: u64,
    expires_after_ms: u64,
) -> AgentInfo {
    use holochain_p2p::{AgentPubKeyExt, DnaHashExt};
    AgentInfo::new(
        space.into_kitsune_raw(),
        agent.into_kitsune_raw(),
        urls,
        signed_at_ms,
        expires_after_ms,
    )
}

async fn agent_info_to_bytes(info: &AgentInfo) -> PeerDataResult<Vec<u8>> {
    let mut data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut data, info)?;
    Ok(data)
}

async fn sign_agent_info(
    agent: &AgentPubKey,
    data: &[u8],
    keystore: &KeystoreSender,
) -> PeerDataResult<Signature> {
    Ok(sign_agent_info_data(agent, data, keystore).await?)
}

async fn sign_agent_info_data(
    agent: &AgentPubKey,
    data: &[u8],
    keystore: &KeystoreSender,
) -> PeerDataResult<Signature> {
    Ok(agent.sign_raw(keystore, data).await?)
}

#[allow(missing_docs)]
pub mod error {
    use std::{num::TryFromIntError, time::SystemTimeError};

    use kitsune_p2p::KitsuneP2pError;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum PeerDataError {
        #[error(transparent)]
        IoError(#[from] std::io::Error),
        #[error(transparent)]
        KeystoreError(#[from] holochain_keystore::KeystoreError),
        #[error(transparent)]
        TryFromIntError(#[from] TryFromIntError),
        #[error(transparent)]
        SystemTimeError(#[from] SystemTimeError),
        #[error(transparent)]
        KitsuneP2pError(#[from] KitsuneP2pError),
    }

    pub type PeerDataResult<T> = Result<T, PeerDataError>;
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, sync::Arc};

    use super::*;
    use fallible_iterator::FallibleIterator;
    use fixt::prelude::*;
    use holo_hash::{fixt::*, AgentPubKey};
    use holochain_state::{
        fresh_reader_test,
        test_utils::{test_p2p_env, TestEnvironment},
    };
    use holochain_zome_types::test_utils::fake_agent_pubkey_1;
    use kitsune_p2p::{fixt::*, KitsuneAgent};

    #[tokio::test(threaded_scheduler)]
    async fn add_agent_info_to_peer_env() {
        observability::test_run().ok();
        let TestEnvironment { env, tmpdir: _t } = test_p2p_env();
        let p2p_store = AgentKv::new(env.clone().into()).unwrap();

        // - Check no data in the store to start
        let count = fresh_reader_test!(env, |r| p2p_store
            .as_store_ref()
            .iter(&r)
            .unwrap()
            .count()
            .unwrap());

        assert_eq!(count, 0);

        // - Get agents and space
        let alice = fake_agent_pubkey_1();
        let some_agent = AgentPubKey::new_from_pure_entropy(env.keystore())
            .await
            .unwrap();
        let space = fixt!(DnaHash);
        let urls = fixt!(Urls);
        let ttl = std::time::Duration::from_secs(60 * 60);

        // - Sign the data
        let alice_signed_agent_info = create_signed_agent_info(
            alice.clone(),
            space.clone(),
            urls.clone(),
            Some(ttl.clone()),
            env.keystore(),
        )
        .await
        .unwrap();
        let some_signed_agent_info = create_signed_agent_info(
            some_agent.clone(),
            space.clone(),
            urls,
            Some(ttl),
            env.keystore(),
        )
        .await
        .unwrap();

        // - Inject some data
        inject_peer_data(env.clone(), alice_signed_agent_info).unwrap();
        inject_peer_data(env.clone(), some_signed_agent_info).unwrap();

        // - Check the same data is now in the store
        let agents = fresh_reader_test!(env, |r| p2p_store
            .as_store_ref()
            .iter(&r)
            .unwrap()
            .map(|(_, v)| Ok(v))
            .collect::<Vec<_>>()
            .unwrap());

        assert_eq!(agents.len(), 2);
        let agents = agents
            .into_iter()
            .map(|ai| {
                use holochain_p2p::AgentPubKeyExt;
                AgentPubKey::from_kitsune(&Arc::new(KitsuneAgent::from(ai)))
            })
            .collect::<HashSet<_>>();
        assert!(agents.contains(&alice));
        assert!(agents.contains(&some_agent));
    }
}
