//! A simple KvBuf for AgentInfoSigned.

use fallible_iterator::FallibleIterator;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_conductor_api::AgentInfoDump;
use holochain_conductor_api::P2pStateDump;
use holochain_p2p::dht_arc::DhtArc;
use holochain_p2p::dht_arc::DhtArcBucket;
use holochain_p2p::dht_arc::PeerDensity;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::CellId;
use kitsune_p2p::{agent_store::AgentInfo, KitsuneBinType};
use std::convert::TryFrom;
use std::convert::TryInto;
use std::sync::Arc;

use super::error::ConductorError;
use super::error::ConductorResult;

const AGENT_KEY_LEN: usize = 64;
const AGENT_KEY_COMPONENT_LEN: usize = 32;

#[derive(Clone)]
/// Required new type for KvBuf key.
pub struct AgentKvKey([u8; AGENT_KEY_LEN]);

impl PartialEq for AgentKvKey {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl std::fmt::Debug for AgentKvKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.0[..])
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

impl std::convert::TryFrom<&AgentInfoSigned> for AgentKvKey {
    type Error = holochain_sqlite::error::DatabaseError;
    fn try_from(agent_info_signed: &AgentInfoSigned) -> Result<Self, Self::Error> {
        let agent_info: AgentInfo = agent_info_signed
            .try_into()
            .map_err(|_| holochain_sqlite::error::DatabaseError::KeyConstruction)?;
        Ok((&agent_info).into())
    }
}

impl From<&AgentInfo> for AgentKvKey {
    fn from(o: &AgentInfo) -> Self {
        (o.as_space_ref(), o.as_agent_ref()).into()
    }
}

impl From<(DnaHash, AgentPubKey)> for AgentKvKey {
    fn from((space, agent): (DnaHash, AgentPubKey)) -> Self {
        let space = holochain_p2p::space_holo_to_kit(space);
        let agent = holochain_p2p::agent_holo_to_kit(agent);
        (&space, &agent).into()
    }
}

impl From<(&kitsune_p2p::KitsuneSpace, &kitsune_p2p::KitsuneAgent)> for AgentKvKey {
    fn from(o: (&kitsune_p2p::KitsuneSpace, &kitsune_p2p::KitsuneAgent)) -> Self {
        let mut bytes = [0; AGENT_KEY_LEN];
        bytes[..AGENT_KEY_COMPONENT_LEN].copy_from_slice(&o.0.get_bytes());
        bytes[AGENT_KEY_COMPONENT_LEN..].copy_from_slice(&o.1.get_bytes());
        Self(bytes)
    }
}

impl From<&[u8]> for AgentKvKey {
    fn from(f: &[u8]) -> Self {
        let mut o = [0_u8; AGENT_KEY_LEN];
        o.copy_from_slice(&f[..AGENT_KEY_LEN]);
        Self(o)
    }
}

impl From<Vec<u8>> for AgentKvKey {
    fn from(v: Vec<u8>) -> Self {
        v.as_slice().into()
    }
}

impl AsRef<[u8]> for AgentKvKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

holochain_zome_types::impl_to_sql_via_as_ref!(AgentKvKey);

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
    /// Constructor.
    pub fn new(env: EnvRead) -> DatabaseResult<Self> {
        let db = env.get_table(TableName::Agent)?;
        Ok(Self(KvStore::new(db)))
    }

    /// Thin AsRef wrapper for the inner store.
    pub fn as_store_ref(&self) -> &KvStore<AgentKvKey, AgentInfoSigned> {
        self.as_ref()
    }

    /// Get a single agent info from the database
    pub fn get_agent_info<'r, R: Readable>(
        &'r self,
        reader: &'r mut R,
        space: DnaHash,
        agent: AgentPubKey,
    ) -> DatabaseResult<Option<AgentInfoSigned>> {
        let key: AgentKvKey = (space, agent).into();
        self.0.get(reader, &key)
    }

    /// Get an iterator of the agent info stored in this database.
    pub fn iter<'r, R: Readable>(
        &'r self,
        reader: &'r mut R,
    ) -> DatabaseResult<
        impl FallibleIterator<Item = (AgentKvKey, AgentInfoSigned), Error = DatabaseError> + 'r,
    > {
        Ok(self
            .as_store_ref()
            .iter(reader)?
            .map(|(k, v)| Ok((k.into(), v))))
    }
}

/// Inject multiple agent info entries into the peer store
pub fn inject_agent_infos<I: IntoIterator<Item = AgentInfoSigned> + Send>(
    env: EnvWrite,
    iter: I,
) -> DatabaseResult<()> {
    let p2p_store = AgentKv::new(env.clone().into())?;
    Ok(env.conn()?.with_commit(|writer| {
        for agent_info_signed in iter {
            p2p_store.as_store_ref().put(
                writer,
                &(&agent_info_signed).try_into()?,
                &agent_info_signed,
            )?
        }
        DatabaseResult::Ok(())
    })?)
}

/// Helper function to get all the peer data from this conductor
pub fn all_agent_infos(env: EnvRead) -> DatabaseResult<Vec<AgentInfoSigned>> {
    let p2p_store = AgentKv::new(env.clone())?;
    fresh_reader!(env, |mut r| {
        p2p_store.iter(&mut r)?.map(|(_, v)| Ok(v)).collect()
    })
}

/// Helper function to get a single agent info
pub fn get_single_agent_info(
    env: EnvRead,
    space: DnaHash,
    agent: AgentPubKey,
) -> DatabaseResult<Option<AgentInfoSigned>> {
    let p2p_store = AgentKv::new(env.clone())?;
    fresh_reader!(env, |mut r| {
        p2p_store.get_agent_info(&mut r, space, agent)
    })
}

/// Interconnect every provided pair of conductors via their peer store databases
#[cfg(any(test, feature = "test_utils"))]
pub fn exchange_peer_info(envs: Vec<EnvWrite>) {
    for (i, a) in envs.iter().enumerate() {
        for (j, b) in envs.iter().enumerate() {
            if i == j {
                continue;
            }
            inject_agent_infos(a.clone(), all_agent_infos(b.clone().into()).unwrap()).unwrap();
            inject_agent_infos(b.clone(), all_agent_infos(a.clone().into()).unwrap()).unwrap();
        }
    }
}

/// Get agent info for a single agent
pub fn get_agent_info_signed(
    environ: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
) -> ConductorResult<Option<AgentInfoSigned>> {
    let p2p_kv = AgentKv::new(environ.clone().into())?;

    environ.conn()?.with_commit(|writer| {
        let res = p2p_kv
            .as_store_ref()
            .get(writer, &(&*kitsune_space, &*kitsune_agent).into())?;

        let res = match res {
            None => return Ok(None),
            Some(res) => res,
        };

        let info = kitsune_p2p::agent_store::AgentInfo::try_from(&res)?;
        let now = now();

        if is_expired(now, &info) {
            p2p_kv
                .as_store_ref()
                .delete(writer, &(&*kitsune_space, &*kitsune_agent).into())?;
            return Ok(None);
        }

        Ok(Some(res))
    })
}

/// Get agent info for a single space
pub fn query_agent_info_signed(
    environ: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
) -> ConductorResult<Vec<AgentInfoSigned>> {
    let p2p_kv = AgentKv::new(environ.clone().into())?;

    let mut out = Vec::new();
    environ.conn()?.with_commit(|writer| {
        let mut expired = Vec::new();

        {
            let mut iter = p2p_kv.as_store_ref().iter(writer)?;

            let now = now();

            loop {
                match iter.next() {
                    Ok(Some((k, v))) => {
                        let info = kitsune_p2p::agent_store::AgentInfo::try_from(&v)?;
                        if is_expired(now, &info) {
                            expired.push(AgentKvKey::from(k));
                        } else if info.as_space_ref() == kitsune_space.as_ref() {
                            out.push(v);
                        }
                    }
                    Ok(None) => break,
                    Err(e) => return Err(e.into()),
                }
            }
        }

        if !expired.is_empty() {
            for exp in expired {
                p2p_kv.as_store_ref().delete(writer, &exp)?;
            }
        }

        ConductorResult::Ok(())
    })?;

    Ok(out)
}

/// Get the peer density an agent is currently seeing within
/// a given [`DhtArc`]
pub fn query_peer_density(
    env: EnvWrite,
    kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    dht_arc: DhtArc,
) -> ConductorResult<PeerDensity> {
    let p2p_store = AgentKv::new(env.clone().into())?;
    let now = now();
    let arcs = fresh_reader!(env, |mut r| {
        p2p_store
            .iter(&mut r)?
            .map(|(_, v)| Ok(v))
            .map_err(ConductorError::from)
            .filter_map(|v| {
                if dht_arc.contains(v.as_agent_ref().get_loc()) {
                    let info = kitsune_p2p::agent_store::AgentInfo::try_from(&v)?;
                    if info.as_space_ref() == kitsune_space.as_ref() && !is_expired(now, &info) {
                        Ok(Some(info.dht_arc()?))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            })
            .collect()
    })?;

    // contains is already checked in the iterator
    let bucket = DhtArcBucket::new_unchecked(dht_arc, arcs);

    Ok(bucket.density())
}

/// Put single agent info into store
pub fn put_agent_info_signed(
    environ: EnvWrite,
    agent_info_signed: kitsune_p2p::agent_store::AgentInfoSigned,
) -> ConductorResult<()> {
    let p2p_kv = AgentKv::new(environ.clone().into())?;
    Ok(environ.conn()?.with_commit(|writer| {
        p2p_kv.as_store_ref().put(
            writer,
            &(&agent_info_signed).try_into()?,
            &agent_info_signed,
        )
    })?)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn is_expired(now: u64, info: &kitsune_p2p::agent_store::AgentInfo) -> bool {
    info.signed_at_ms()
        .checked_add(info.expires_after_ms())
        .map(|expires| expires <= now)
        .unwrap_or(true)
}

/// Dump the agents currently in the peer store
pub fn dump_state(env: EnvRead, cell_id: Option<CellId>) -> DatabaseResult<P2pStateDump> {
    use std::fmt::Write;
    let cell_id = cell_id.map(|c| c.into_dna_and_agent()).map(|c| {
        (
            (c.0.clone(), holochain_p2p::space_holo_to_kit(c.0)),
            (c.1.clone(), holochain_p2p::agent_holo_to_kit(c.1)),
        )
    });
    let agent_infos = all_agent_infos(env)?;
    let agent_infos =
        agent_infos.into_iter().filter_map(
            |a| match kitsune_p2p::agent_store::AgentInfo::try_from(&a) {
                Ok(a) => match &cell_id {
                    Some((s, _)) => {
                        if s.1 == *a.as_space_ref() {
                            Some(a)
                        } else {
                            None
                        }
                    }
                    None => Some(a),
                },
                Err(e) => {
                    tracing::error!(failed_to_deserialize_agent_info = ?e);
                    None
                }
            },
        );
    let mut this_agent_info = None;
    let mut peers = Vec::new();
    for info in agent_infos {
        let mut dump = String::new();

        use chrono::{DateTime, Duration, NaiveDateTime, Utc};
        let duration = Duration::milliseconds(info.signed_at_ms() as i64);
        let s = duration.num_seconds() as i64;
        let n = duration.clone().to_std().unwrap().subsec_nanos();
        let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(s, n), Utc);
        let exp = dt + Duration::milliseconds(info.expires_after_ms() as i64);
        let now = Utc::now();

        writeln!(dump, "signed at {}", dt).ok();
        writeln!(
            dump,
            "expires at {} in {}mins",
            exp,
            (exp - now).num_minutes()
        )
        .ok();
        writeln!(dump, "urls: {:?}", info.as_urls_ref()).ok();
        let info = AgentInfoDump {
            kitsune_agent: info.as_agent_ref().clone(),
            kitsune_space: info.as_space_ref().clone(),
            dump,
        };
        match &cell_id {
            Some((s, a)) if info.kitsune_agent == a.1 && info.kitsune_space == s.1 => {
                this_agent_info = Some(info);
            }
            None | Some(_) => peers.push(info),
        }
    }

    Ok(P2pStateDump {
        this_agent_info,
        this_dna: cell_id.clone().map(|(s, _)| s),
        this_agent: cell_id.clone().map(|(_, a)| a),
        peers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holochain_sqlite::buffer::KvStoreT;
    use holochain_sqlite::db::ReadManager;
    use holochain_sqlite::db::WriteManager;
    use holochain_sqlite::fresh_reader_test;
    use holochain_state::test_utils::test_p2p_env;
    use kitsune_p2p::fixt::AgentInfoFixturator;
    use kitsune_p2p::fixt::AgentInfoSignedFixturator;
    use kitsune_p2p::KitsuneBinType;
    use std::convert::TryInto;

    #[test]
    fn kv_key_from() {
        let agent_info = fixt!(AgentInfo);

        let kv_key = AgentKvKey::from(&agent_info);

        let bytes = kv_key.as_ref().to_owned();

        assert_eq!(&bytes[..32], agent_info.as_space_ref().get_bytes(),);

        assert_eq!(&bytes[32..], agent_info.as_agent_ref().get_bytes(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_store_agent_info_signed() {
        observability::test_run().ok();

        let test_env = test_p2p_env();
        let environ = test_env.env();

        let store_buf = super::AgentKv::new(environ.clone().into()).unwrap();

        let agent_info_signed = fixt!(AgentInfoSigned);

        environ
            .conn()
            .unwrap()
            .with_commit(|writer| {
                store_buf.as_store_ref().put(
                    writer,
                    &(&agent_info_signed).try_into().unwrap(),
                    &agent_info_signed,
                )
            })
            .unwrap();

        environ.conn().unwrap().with_reader_test(|mut reader| {
            let ret = &store_buf
                .as_store_ref()
                .get(&mut reader, &(&agent_info_signed).try_into().unwrap())
                .unwrap();

            assert_eq!(ret, &Some(agent_info_signed),);
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_agent_info_to_peer_env() {
        observability::test_run().ok();
        let t_env = test_p2p_env();
        let env = t_env.env();
        let p2p_store = AgentKv::new(env.clone().into()).unwrap();

        // - Check no data in the store to start
        let count = fresh_reader_test!(env, |mut r| p2p_store
            .as_store_ref()
            .iter(&mut r)
            .unwrap()
            .count()
            .unwrap());

        assert_eq!(count, 0);

        // - Get agents and space
        let agent_infos = AgentInfoSignedFixturator::new(Unpredictable)
            .take(5)
            .collect::<Vec<_>>();

        let mut expect = agent_infos.clone();
        expect.sort();

        // - Inject some data
        inject_agent_infos(env.clone(), agent_infos).unwrap();

        // - Check the same data is now in the store
        let mut agents = all_agent_infos(env.clone().into()).unwrap();

        agents.sort();

        assert_eq!(expect, agents);
    }
}
