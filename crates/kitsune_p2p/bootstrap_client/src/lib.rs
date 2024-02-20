use kitsune_p2p_bootstrap::error::BootstrapClientError;
use kitsune_p2p_bootstrap::error::BootstrapClientResult;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::RandomQuery;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use std::convert::TryFrom;
use std::convert::TryInto;
use url2::Url2;

pub mod prelude {
    pub use super::{now, now_once, proxy_list, put, random, BootstrapNet};
    pub use kitsune_p2p_bootstrap::error::*;
}

/// The "net" flag / bucket to use when talking to the bootstrap server.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BootstrapNet {
    Tx2,
    Tx5,
}

impl BootstrapNet {
    fn value(&self) -> &'static str {
        match self {
            BootstrapNet::Tx2 => "tx2",
            BootstrapNet::Tx5 => "tx5",
        }
    }
}

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
    net: BootstrapNet,
) -> BootstrapClientResult<Option<O>> {
    let mut body_data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut body_data, &input)?;
    match url {
        Some(url) => {
            let url = format!("{}?net={}", url.as_str(), net.value());

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
                Err(BootstrapClientError::Bootstrap(
                    res.text().await?.into_boxed_str(),
                ))
            }
        }
        None => Ok(None),
    }
}

/// `do_api` wrapper for the `put` op.
///
/// Input must be an AgentInfoSigned with a valid signature otherwise the remote service will not
/// accept the data.
pub async fn put(
    url: Option<Url2>,
    agent_info_signed: AgentInfoSigned,
    net: BootstrapNet,
) -> BootstrapClientResult<()> {
    match do_api(url, OP_PUT, agent_info_signed, net).await {
        Ok(Some(())) => Ok(()),
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Simple wrapper to get the local time as milliseconds, to be compared against the remote time.
fn local_now() -> BootstrapClientResult<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

/// Thin `do_api` wrapper for the `now` op.
///
/// There is no input to the `now` endpoint, just `()` to be encoded as nil in messagepack.
#[allow(dead_code)]
pub async fn now(url: Option<Url2>, net: BootstrapNet) -> BootstrapClientResult<u64> {
    match do_api(url, OP_NOW, (), net).await {
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
pub async fn now_once(url: Option<Url2>, net: BootstrapNet) -> BootstrapClientResult<u64> {
    match NOW_OFFSET_MILLIS.get() {
        Some(offset) => Ok(u64::try_from(i64::try_from(local_now()?)? + offset)?),
        None => {
            let offset: i64 = match now(url.clone(), net).await {
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
    net: BootstrapNet,
) -> BootstrapClientResult<Vec<AgentInfoSigned>> {
    let outer_vec: Vec<serde_bytes::ByteBuf> = match do_api(url, OP_RANDOM, query, net).await {
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
pub async fn proxy_list(url: Url2, net: BootstrapNet) -> BootstrapClientResult<Vec<Url2>> {
    Ok(do_api::<_, Vec<String>>(Some(url), OP_PROXY_LIST, (), net)
        .await?
        .unwrap_or_default()
        .into_iter()
        .flat_map(|s| Url2::try_parse(s).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use ed25519_dalek::ed25519::signature::Signature;
    use ed25519_dalek::Keypair;
    use ed25519_dalek::Signer;
    use kitsune_p2p_bin_data::fixt::*;
    use kitsune_p2p_bin_data::KitsuneAgent;
    use kitsune_p2p_bin_data::KitsuneBinType;
    use kitsune_p2p_bin_data::KitsuneSignature;
    use kitsune_p2p_types::fixt::*;
    use std::convert::TryInto;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::task::AbortHandle;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_bootstrap() {
        let (addr, abort_handle) = start_bootstrap().await;

        let keypair = create_test_keypair();
        let space = fixt!(KitsuneSpace);
        let agent = KitsuneAgent::new(keypair.public.as_bytes().to_vec());
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
                async move {
                    Ok(Arc::new(KitsuneSignature(
                        keypair.sign(d.clone().as_slice()).as_bytes().to_vec(),
                    )))
                }
            },
        )
        .await
        .unwrap();

        // Simply hitting the endpoint should be OK.
        put(
            Some(url2::url2!("http://{:?}", addr)),
            agent_info_signed,
            BootstrapNet::Tx5,
        )
        .await
        .unwrap();

        // We should get back an error if we don't have a good signature.
        let bad = fixt!(AgentInfoSigned);
        let mut bad = Arc::try_unwrap(bad.0).unwrap();
        bad.signature = Arc::new(vec![].into());
        let bad = AgentInfoSigned(Arc::new(bad));
        let res = put(
            Some(url2::url2!("http://{:?}", addr)),
            bad,
            BootstrapNet::Tx5,
        )
        .await;
        assert!(res.is_err());

        abort_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_now() {
        let (addr, abort_handle) = start_bootstrap().await;

        let local_now = std::time::SystemTime::now();
        let local_millis: u64 = local_now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap();

        // We should be able to get a milliseconds timestamp back.
        let remote_now: u64 = now(Some(url2::url2!("http://{:?}", addr)), BootstrapNet::Tx5)
            .await
            .unwrap();
        let threshold = 5000;

        assert!((remote_now - local_millis) < threshold);

        // Now once should return some number and the remote server offset should be set in the
        // NOW_OFFSET_MILLIS once cell.
        let _: u64 = now_once(Some(url2::url2!("http://{:?}", addr)), BootstrapNet::Tx5)
            .await
            .unwrap();
        assert!(NOW_OFFSET_MILLIS.get().is_some());

        abort_handle.abort();
    }

    #[tokio::test(flavor = "multi_thread")]
    // Fixturator seed: 17591570467001263546
    // thread 'spawn::actor::bootstrap::tests::test_random' panicked at 'dispatch dropped without returning error', /rustc/d3fb005a39e62501b8b0b356166e515ae24e2e54/src/libstd/macros.rs:13:23
    async fn test_random() {
        let (addr, abort_handle) = start_bootstrap().await;

        let space = fixt!(KitsuneSpace, Unpredictable);
        let now = now(Some(url2::url2!("http://{:?}", addr)), BootstrapNet::Tx5)
            .await
            .unwrap();

        let alice = create_test_keypair();
        let bob = create_test_keypair();

        let mut expected: Vec<AgentInfoSigned> = Vec::new();
        for agent in vec![alice, bob] {
            let kitsune_agent = KitsuneAgent::new(agent.public.as_bytes().to_vec());
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
                    async move {
                        Ok(Arc::new(KitsuneSignature(
                            agent.sign(d.clone().as_slice()).as_bytes().to_vec(),
                        )))
                    }
                },
            )
            .await
            .unwrap();

            put(
                Some(url2::url2!("http://{:?}", addr)),
                agent_info_signed.clone(),
                BootstrapNet::Tx5,
            )
            .await
            .unwrap();

            expected.push(agent_info_signed);
        }

        let mut random = super::random(
            Some(url2::url2!("http://{:?}", addr)),
            RandomQuery {
                space: Arc::new(space.clone()),
                ..Default::default()
            },
            BootstrapNet::Tx2,
        )
        .await
        .unwrap();

        expected.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());
        random.sort_by(|a, b| a.agent.partial_cmp(&b.agent).unwrap());

        assert!(random.len() == 2);
        assert!(random == expected);

        let random_single = super::random(
            Some(url2::url2!("http://{:?}", addr)),
            RandomQuery {
                space: Arc::new(space.clone()),
                limit: 1.into(),
            },
            BootstrapNet::Tx5,
        )
        .await
        .unwrap();

        assert!(random_single.len() == 1);
        assert!(expected[0] == random_single[0] || expected[1] == random_single[0]);

        abort_handle.abort();
    }

    async fn start_bootstrap() -> (SocketAddr, AbortHandle) {
        let (bs_driver, bs_addr, shutdown) =
            kitsune_p2p_bootstrap::run("127.0.0.1:0".parse::<SocketAddr>().unwrap(), vec![])
                .await
                .expect("Could not start bootstrap server");

        let abort_handle = tokio::spawn(async move {
            let _shutdown_cb = shutdown;
            bs_driver.await;
        })
        .abort_handle();

        (bs_addr, abort_handle)
    }

    fn create_test_keypair() -> Keypair {
        let mut rng = rand_dalek::thread_rng();
        Keypair::generate(&mut rng)
    }
}
