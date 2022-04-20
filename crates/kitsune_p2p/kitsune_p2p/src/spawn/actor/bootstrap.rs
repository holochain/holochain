use crate::types::actor::KitsuneP2pResult;
use crate::types::agent_store::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::RandomQuery;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use std::convert::TryFrom;
use std::convert::TryInto;
use url2::Url2;

/// Reuse a single reqwest Client for efficiency as we likely need several connections.
static CLIENT: Lazy<reqwest::Client> = Lazy::new(reqwest::Client::new);

/// A cell to hold our local offset for calculating a 'now' that is compatible with the remote
/// service. This is much less precise and comprehensive than NTP style calculations.
/// We simply need to ensure that we don't sign things 'in the future' from the perspective of the
/// remote service, and any inaccuracy caused by network latency or similar problems is negligible
/// relative to the expiry times.
pub static NOW_OFFSET_MILLIS: OnceCell<i64> = OnceCell::new();

/// The HTTP header name for setting the op on POST requests.
const OP_HEADER: &str = "X-Op";
/// The header op to tell the service to put a signed agent info.
const OP_PUT: &str = "put";
/// The header op to tell the service to return its opinion of 'now' in milliseconds.
const OP_NOW: &str = "now";
/// The header op to tell the service to return a random set of agents in a specific space.
const OP_RANDOM: &str = "random";
/// The header op to fetch the proxy_list from the bootstrap service
const OP_PROXY_LIST: &str = "proxy_list";

/// Standard interface to the remote bootstrap service.
///
/// - url: the url of the bootstrap service or None to short circuit and not send a request
/// - op: the header op for the remote service
/// - input: op-specific struct that will be messagepack encoded and sent as binary data in the
///          body of the POST
///
/// Output type O is op specific and needs to be messagepack decodeable.
async fn do_api<I: serde::Serialize, O: serde::de::DeserializeOwned>(
    url: Option<Url2>,
    op: &str,
    input: I,
) -> crate::types::actor::KitsuneP2pResult<Option<O>> {
    let mut body_data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut body_data, &input)?;
    match url {
        Some(url) => {
            let res = CLIENT
                .post(url.as_str())
                .body(body_data)
                .header(OP_HEADER, op)
                .header(reqwest::header::CONTENT_TYPE, "application/octet")
                .send()
                .await?;
            if res.status().is_success() {
                Ok(Some(kitsune_p2p_types::codec::rmp_decode(
                    &mut res.bytes().await?.as_ref(),
                )?))
            } else {
                Err(crate::KitsuneP2pError::Bootstrap(
                    res.text().await?.into_boxed_str(),
                ))
            }
        }
        None => Ok(None),
    }
}

/// `do_api` wrapper for the `put` op.
///
/// Input must be an AgentInfoSigned with a valid siganture otherwise the remote service will not
/// accept the data.
pub async fn put(
    url: Option<Url2>,
    agent_info_signed: crate::types::agent_store::AgentInfoSigned,
) -> crate::types::actor::KitsuneP2pResult<()> {
    match do_api(url, OP_PUT, agent_info_signed).await {
        Ok(Some(())) => Ok(()),
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Simple wrapper to get the local time as milliseconds, to be compared against the remote time.
fn local_now() -> crate::types::actor::KitsuneP2pResult<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

/// Thin `do_api` wrapper for the `now` op.
///
/// There is no input to the `now` endpoint, just `()` to be encoded as nil in messagepack.
#[allow(dead_code)]
pub async fn now(url: Option<Url2>) -> crate::types::actor::KitsuneP2pResult<u64> {
    match do_api(url, OP_NOW, ()).await {
        // If the server gives us something useful we use it.
        Ok(Some(v)) => Ok(v),
        // If we don't have a server url we should trust ourselves.
        Ok(None) => Ok(local_now()?),
        // Any error from the server should be handled by the caller.
        // The caller will probably fallback to the local_now.
        Err(e) => Err(e),
    }
}

/// Thick wrapper around `do_api` for the `now` op.
///
/// Calculates the offset on the first call and caches it in the cell above.
/// Only calls `now` once then keeps the offset for the static lifetime.
pub async fn now_once(url: Option<Url2>) -> crate::types::actor::KitsuneP2pResult<u64> {
    match NOW_OFFSET_MILLIS.get() {
        Some(offset) => Ok(u64::try_from(i64::try_from(local_now()?)? + offset)?),
        None => {
            let offset: i64 = match now(url.clone()).await {
                Ok(v) => {
                    let offset = v as i64 - local_now()? as i64;
                    match NOW_OFFSET_MILLIS.set(offset) {
                        Ok(_) => offset,
                        Err(v) => v,
                    }
                }
                // @todo Do something more sophisticated here with errors.
                // Currently just falls back to a zero offset if the server is not happy.
                Err(_) => {
                    let offset = 0;
                    match NOW_OFFSET_MILLIS.set(offset) {
                        Ok(_) => offset,
                        Err(v) => v,
                    }
                }
            };

            Ok(u64::try_from(i64::try_from(local_now()?)? + offset)?)
        }
    }
}

/// `do_api` wrapper around the `random` op.
///
/// Fetches up to `limit` agent infos randomly from the `space`.
///
/// If there are fewer than `limit` agents listing themselves in the space then `limit` agents will
/// be returned in a random order.
///
/// The ordering is random, the return is not sorted.
/// Randomness is determined by the bootstrap service, it is one of the important roles of the
/// service to mitigate eclipse attacks by having a strong randomness implementation.
#[allow(dead_code)]
pub async fn random(
    url: Option<Url2>,
    query: RandomQuery,
) -> crate::types::actor::KitsuneP2pResult<Vec<AgentInfoSigned>> {
    let outer_vec: Vec<serde_bytes::ByteBuf> = match do_api(url, OP_RANDOM, query).await {
        Ok(Some(v)) => v,
        Ok(None) => Vec::new(),
        Err(e) => return Err(e),
    };
    let ret: Result<Vec<AgentInfoSigned>, _> = outer_vec
        .into_iter()
        .map(|bytes| kitsune_p2p_types::codec::rmp_decode(&mut AsRef::<[u8]>::as_ref(&bytes)))
        .collect();
    Ok(ret?)
}

/// `do_api` wrapper around the `proxy_list` op.
///
/// Fetches the list of proxy servers currently stored in the bootstrap service.
#[allow(dead_code)]
pub async fn proxy_list(url: Url2) -> KitsuneP2pResult<Vec<Url2>> {
    Ok(do_api::<_, Vec<String>>(Some(url), OP_PROXY_LIST, ())
        .await?
        .unwrap_or_default()
        .into_iter()
        .map(Url2::parse)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixt::*;
    use crate::types::KitsuneAgent;
    use crate::types::KitsuneBinType;
    use crate::types::KitsuneSignature;
    use ::fixt::prelude::*;
    use kitsune_p2p_types::dependencies::lair_keystore_api_0_0::internal::sign_ed25519::sign_ed25519_keypair_new_from_entropy;
    use kitsune_p2p_types::KitsuneError;
    use std::convert::TryInto;
    use std::sync::Arc;

    // TODO - FIXME - davidb
    // I'm disabling all these tests that depend on outside systems
    // we need local testing to prove these out in a ci environment.

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "flaky"]
    async fn test_bootstrap() {
        let keypair = sign_ed25519_keypair_new_from_entropy().await.unwrap();
        let space = fixt!(KitsuneSpace);
        let agent = KitsuneAgent::new((*keypair.pub_key.0).clone());
        let urls = fixt!(UrlList);
        let now = std::time::SystemTime::now();
        let millis = now
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        let signed_at_ms = (millis - 100).try_into().unwrap();
        let expires_at_ms = signed_at_ms + 1000 * 60 * 20;
        let agent_info_signed = AgentInfoSigned::sign(
            Arc::new(space),
            Arc::new(agent),
            u32::MAX,
            urls,
            signed_at_ms,
            expires_at_ms,
            |d| {
                let d = Arc::new(d.to_vec());
                async {
                    keypair
                        .sign(d)
                        .await
                        .map(|s| Arc::new(KitsuneSignature(s.0.to_vec())))
                        .map_err(KitsuneError::other)
                }
            },
        )
        .await
        .unwrap();

        // Simply hitting the endpoint should be OK.
        super::put(
            Some(url2::url2!("{}", crate::config::BOOTSTRAP_SERVICE_DEV)),
            agent_info_signed,
        )
        .await
        .unwrap();

        // We should get back an error if we don't have a good signature.
        assert!(super::put(
            Some(url2::url2!("{}", crate::config::BOOTSTRAP_SERVICE_DEV)),
            fixt!(AgentInfoSigned)
        )
        .await
        .is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "flaky"]
    async fn test_now() {
        let local_now = std::time::SystemTime::now();
        let local_millis: u64 = local_now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap();

        // We should be able to get a milliseconds timestamp back.
        let remote_now: u64 = super::now(Some(url2::url2!(
            "{}",
            crate::config::BOOTSTRAP_SERVICE_DEV
        )))
        .await
        .unwrap();
        let threshold = 5000;

        assert!((remote_now - local_millis) < threshold);

        // Now once should return some number and the remote server offset should be set in the
        // NOW_OFFSET_MILLIS once cell.
        let _: u64 = super::now_once(Some(url2::url2!(
            "{}",
            crate::config::BOOTSTRAP_SERVICE_DEV
        )))
        .await
        .unwrap();
        assert!(super::NOW_OFFSET_MILLIS.get().is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "flaky"]
    // Fixturator seed: 17591570467001263546
    // thread 'spawn::actor::bootstrap::tests::test_random' panicked at 'dispatch dropped without returning error', /rustc/d3fb005a39e62501b8b0b356166e515ae24e2e54/src/libstd/macros.rs:13:23
    async fn test_random() {
        let space = fixt!(KitsuneSpace, Unpredictable);
        let now = super::now(Some(url2::url2!(
            "{}",
            crate::config::BOOTSTRAP_SERVICE_DEV
        )))
        .await
        .unwrap();

        let alice = sign_ed25519_keypair_new_from_entropy().await.unwrap();
        let bob = sign_ed25519_keypair_new_from_entropy().await.unwrap();

        let mut expected: Vec<AgentInfoSigned> = Vec::new();
        for agent in vec![alice.clone(), bob.clone()] {
            let kitsune_agent = KitsuneAgent::new((*agent.pub_key.0).clone());
            let signed_at_ms = now;
            let expires_at_ms = now + 1000 * 60 * 20;
            let agent_info_signed = AgentInfoSigned::sign(
                Arc::new(space.clone()),
                Arc::new(kitsune_agent.clone()),
                u32::MAX,
                fixt!(UrlList),
                signed_at_ms,
                expires_at_ms,
                |d| {
                    let d = Arc::new(d.to_vec());
                    async {
                        agent
                            .sign(d)
                            .await
                            .map(|s| Arc::new(KitsuneSignature(s.0.to_vec())))
                            .map_err(KitsuneError::other)
                    }
                },
            )
            .await
            .unwrap();

            super::put(
                Some(url2::url2!("{}", crate::config::BOOTSTRAP_SERVICE_DEV)),
                agent_info_signed.clone(),
            )
            .await
            .unwrap();

            expected.push(agent_info_signed);
        }

        let mut random = super::random(
            Some(url2::url2!("{}", crate::config::BOOTSTRAP_SERVICE_DEV)),
            super::RandomQuery {
                space: Arc::new(space.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        expected.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());
        random.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());

        assert!(random.len() == 2);
        assert!(random == expected);

        let random_single = super::random(
            Some(url2::url2!("{}", crate::config::BOOTSTRAP_SERVICE_DEV)),
            super::RandomQuery {
                space: Arc::new(space.clone()),
                limit: 1.into(),
            },
        )
        .await
        .unwrap();

        assert!(random_single.len() == 1);
        assert!(expected[0] == random_single[0] || expected[1] == random_single[0]);
    }
}
