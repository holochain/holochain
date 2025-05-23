//! DANGER! This is an in-memory keystore for testing, DO NOT USE THIS IN PRODUCTION!

use crate::*;
use ::lair_keystore::dependencies::lair_keystore_api;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use lair_keystore_api::prelude::*;
use std::sync::{Arc, Mutex};

/// First Test Agent Pub Key
pub const TEST_AGENT_PK_1: &str = "uhCAkJCuynkgVdMn_bzZ2ZYaVfygkn0WCuzfFspczxFnZM1QAyXoo";
const SEED_1: &str = "m-U7gdxW1A647O-4wkuCWOvtGGVfHEsxNScFKiL8-k8";

/// Second Test Agent Pub Key
pub const TEST_AGENT_PK_2: &str = "uhCAk39SDf7rynCg5bYgzroGaOJKGKrloI1o57Xao6S-U5KNZ0dUH";
const SEED_2: &str = "v9I5GT3xVKPcaa4uyd2pcuJromf5zv1-OaahYOLBAWY";

/// Third Test Agent Pub Key
pub const TEST_AGENT_PK_3: &str = "uhCAkwfTgZ5eDJwI6ZV5vGt-kg8cVgXvcf35XKj6HnMv4PBH8noYB";
const SEED_3: &str = "NE_0oUEATrsTR0o7JM1H8I6X6dtXg51iZvtCHAw6Fgg";

/// Fourth Test Agent Pub Key
pub const TEST_AGENT_PK_4: &str = "uhCAkQHMlYam1PRiYJCzAwQ0AUxIMwOoOvxgXS67N_YPOMj-fGx6X";
const SEED_4: &str = "2o79pTXHaK1FTPZeBiJo2lCgXW_P0ULjX_5Div_2qxU";

fn r(s: &str) -> Vec<u8> {
    URL_SAFE_NO_PAD.decode(s).unwrap()
}

fn s(s: &str) -> SharedSizedLockedArray<32> {
    let r_ = r(s);
    let mut o = sodoken::SizedLockedArray::<32>::new().unwrap();
    o.lock().copy_from_slice(&r_);

    Arc::new(Mutex::new(o))
}

/// Construct a new test keystore with the new lair api.
pub async fn spawn_test_keystore() -> LairResult<MetaLairClient> {
    // in-memory secure random passphrase
    let mut passphrase = sodoken::LockedArray::new(32)?;
    sodoken::random::randombytes_buf(&mut passphrase.lock())?;

    let passphrase = Arc::new(Mutex::new(passphrase));

    // in-mem / in-proc config
    let config = Arc::new(
        PwHashLimits::Minimum
            .with_exec(|| {
                lair_keystore_api::config::LairServerConfigInner::new("/", passphrase.clone())
            })
            .await?,
    );

    // the keystore
    let keystore = lair_keystore_api::in_proc_keystore::InProcKeystore::new(
        config,
        lair_keystore_api::mem_store::create_mem_store_factory(),
        passphrase,
    )
    .await?;

    // get the store and inject test seeds
    let store = keystore.store().await?;
    store
        .insert_seed(s(SEED_1), TEST_AGENT_PK_1.into(), false)
        .await?;
    store
        .insert_seed(s(SEED_2), TEST_AGENT_PK_2.into(), false)
        .await?;
    store
        .insert_seed(s(SEED_3), TEST_AGENT_PK_3.into(), false)
        .await?;
    store
        .insert_seed(s(SEED_4), TEST_AGENT_PK_4.into(), false)
        .await?;

    // return the client
    let client = keystore.new_client().await?;
    let (s, _) = tokio::sync::mpsc::unbounded_channel();
    Ok(MetaLairClient(Arc::new(parking_lot::Mutex::new(client)), s))
}

/// Generate a test keystore pre-populated with a couple of test key-pairs.
pub fn test_keystore() -> MetaLairClient {
    holochain_util::tokio_helper::block_on(
        async move { spawn_test_keystore().await.unwrap() },
        std::time::Duration::from_secs(5),
    )
    .expect("timeout elapsed")
}
