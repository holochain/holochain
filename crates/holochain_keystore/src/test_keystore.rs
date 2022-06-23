//! DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!

use crate::*;
use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::prelude::*;
use std::sync::Arc;

/// First Test Agent Pub Key
pub const TEST_AGENT_PK_1: &str = "uhCAkJCuynkgVdMn_bzZ2ZYaVfygkn0WCuzfFspczxFnZM1QAyXoo";
const SEED_1: &str = "m-U7gdxW1A647O-4wkuCWOvtGGVfHEsxNScFKiL8-k8";
const ED_PK_1: &str = "JCuynkgVdMn_bzZ2ZYaVfygkn0WCuzfFspczxFnZM1Q";
//const ED_SK_1: &str =
//    "m-U7gdxW1A647O-4wkuCWOvtGGVfHEsxNScFKiL8-k8kK7KeSBV0yf9vNnZlhpV_KCSfRYK7N8WylzPEWdkzVA";
const X_PK_1: &str = "7RkNzL1Eu9ynrUT9NsqdLFNoGqQVcuOAHBOgzT550BY";
const X_SK_1: &str = "j3qOevzDNH0EPOZVqgq_a3WLU3REJHwtk_N1wSoT900";

/// Second Test Agent Pub Key
pub const TEST_AGENT_PK_2: &str = "uhCAk39SDf7rynCg5bYgzroGaOJKGKrloI1o57Xao6S-U5KNZ0dUH";
const SEED_2: &str = "v9I5GT3xVKPcaa4uyd2pcuJromf5zv1-OaahYOLBAWY";
const ED_PK_2: &str = "39SDf7rynCg5bYgzroGaOJKGKrloI1o57Xao6S-U5KM";
//const ED_SK_2: &str =
//    "v9I5GT3xVKPcaa4uyd2pcuJromf5zv1-OaahYOLBAWbf1IN_uvKcKDltiDOugZo4koYquWgjWjntdqjpL5Tkow";
const X_PK_2: &str = "rrp3HzChuX7ySxFrrwZ-1C91Lz1ygiBMpug1lxd162c";
const X_SK_2: &str = "6AyYjh1sPqiyhgWDToMHbsPNtNZdvPD81QkSDiLQEvg";

/// Third Test Agent Pub Key
pub const TEST_AGENT_PK_3: &str = "uhCAkwfTgZ5eDJwI6ZV5vGt-kg8cVgXvcf35XKj6HnMv4PBH8noYB";
const SEED_3: &str = "NE_0oUEATrsTR0o7JM1H8I6X6dtXg51iZvtCHAw6Fgg";
const ED_PK_3: &str = "wfTgZ5eDJwI6ZV5vGt-kg8cVgXvcf35XKj6HnMv4PBE";
//const ED_SK_3: &str =
//    "NE_0oUEATrsTR0o7JM1H8I6X6dtXg51iZvtCHAw6FgjB9OBnl4MnAjplXm8a36SDxxWBe9x_flcqPoecy_g8EQ";
const X_PK_3: &str = "0j2y0hMh1ka-DIMSqHsEvefwowMxE0pmIyIYL1xSnVE";
const X_SK_3: &str = "fZqDBKw6nQoj7Zn-B9ebFiBs-nY54F6kGdXEFoHnsIg";

/// Fourth Test Agent Pub Key
pub const TEST_AGENT_PK_4: &str = "uhCAkQHMlYam1PRiYJCzAwQ0AUxIMwOoOvxgXS67N_YPOMj-fGx6X";
const SEED_4: &str = "2o79pTXHaK1FTPZeBiJo2lCgXW_P0ULjX_5Div_2qxU";
const ED_PK_4: &str = "QHMlYam1PRiYJCzAwQ0AUxIMwOoOvxgXS67N_YPOMj8";
//const ED_SK_4: &str =
//    "2o79pTXHaK1FTPZeBiJo2lCgXW_P0ULjX_5Div_2qxVAcyVhqbU9GJgkLMDBDQBTEgzA6g6_GBdLrs39g84yPw";
const X_PK_4: &str = "Phplq-vA6Mfs_883RxMeGB_EqWQKkBvNK1atNl7QTnU";
const X_SK_4: &str = "wu4uqLjHoY5RKqRpkKFkskCwdvhp4n91D0tIwzodoX8";

fn r(s: &str) -> Vec<u8> {
    base64::decode_config(s, base64::URL_SAFE_NO_PAD).unwrap()
}

fn s(s: &str) -> [u8; 32] {
    let r_ = r(s);
    let mut o = [0; 32];
    o.copy_from_slice(&r_);
    o
}

/// Construct a new TestKeystore.
/// DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!
pub async fn spawn_legacy_test_keystore() -> KeystoreApiResult<MetaLairClient> {
    use lair_keystore_api_0_0::test::*;
    let (api, _evt) = spawn_test_keystore(
        vec![
            FixtureSignEd25519Keypair {
                pub_key: r(ED_PK_1),
                priv_key: r(SEED_1),
            },
            FixtureSignEd25519Keypair {
                pub_key: r(ED_PK_2),
                priv_key: r(SEED_2),
            },
            FixtureSignEd25519Keypair {
                pub_key: r(ED_PK_3),
                priv_key: r(SEED_3),
            },
            FixtureSignEd25519Keypair {
                pub_key: r(ED_PK_4),
                priv_key: r(SEED_4),
            },
        ],
        vec![],
        vec![
            FixtureX25519Keypair {
                pub_key: s(X_PK_1).into(),
                priv_key: s(X_SK_1).into(),
            },
            FixtureX25519Keypair {
                pub_key: s(X_PK_2).into(),
                priv_key: s(X_SK_2).into(),
            },
            FixtureX25519Keypair {
                pub_key: s(X_PK_3).into(),
                priv_key: s(X_SK_3).into(),
            },
            FixtureX25519Keypair {
                pub_key: s(X_PK_4).into(),
                priv_key: s(X_SK_4).into(),
            },
        ],
    )
    .await?;
    let keystore = MetaLairClient::Legacy(api);
    keystore.new_sign_keypair_random().await.unwrap();
    keystore.new_sign_keypair_random().await.unwrap();
    keystore.new_sign_keypair_random().await.unwrap();
    keystore.new_sign_keypair_random().await.unwrap();
    Ok(keystore)
}

/// Construct a new TestKeystore with the new lair api.
pub async fn spawn_test_keystore() -> LairResult<MetaLairClient> {
    // in-memory secure random passphrase
    let passphrase = sodoken::BufWrite::new_mem_locked(32)?;
    sodoken::random::bytes_buf(passphrase.clone()).await?;

    // in-mem / in-proc config
    let config = Arc::new(
        PwHashLimits::Minimum
            .with_exec(|| {
                lair_keystore_api::config::LairServerConfigInner::new("/", passphrase.to_read())
            })
            .await?,
    );

    // the keystore
    let keystore = lair_keystore_api::in_proc_keystore::InProcKeystore::new(
        config,
        lair_keystore_api::mem_store::create_mem_store_factory(),
        passphrase.to_read(),
    )
    .await?;

    // get the store and inject test seeds
    let store = keystore.store().await?;
    store
        .insert_seed(s(SEED_1).into(), TEST_AGENT_PK_1.into(), false)
        .await?;
    store
        .insert_seed(s(SEED_2).into(), TEST_AGENT_PK_2.into(), false)
        .await?;
    store
        .insert_seed(s(SEED_3).into(), TEST_AGENT_PK_3.into(), false)
        .await?;
    store
        .insert_seed(s(SEED_4).into(), TEST_AGENT_PK_4.into(), false)
        .await?;

    // return the client
    let client = keystore.new_client().await?;
    Ok(MetaLairClient::NewLair(client))
}
