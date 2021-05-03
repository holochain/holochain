use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_p2p::kitsune_p2p;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfo;
use holochain_p2p::kitsune_p2p::agent_store::AgentInfoSigned;
use holochain_p2p::kitsune_p2p::KitsuneBinType;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::ToSql;
use holochain_sqlite::rusqlite::Transaction;

use crate::mutations;
use crate::prelude::from_blob;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;

const AGENT_KEY_LEN: usize = 64;
const AGENT_KEY_COMPONENT_LEN: usize = 32;

#[derive(Clone)]
/// Required new type for KvBuf key.
pub struct AgentKvKey([u8; AGENT_KEY_LEN]);

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
struct Value(AgentInfoSigned);

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

impl ToSql for AgentKvKey {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_ref().into()))
    }
}

pub fn get(txn: &Transaction<'_>, key: AgentKvKey) -> StateQueryResult<Option<AgentInfoSigned>> {
    let item = txn
        .query_row(
            "SELECT blob FROM AgentInfo WHERE key = :key",
            named_params! {
                ":key": key
            },
            |row| {
                let item = row.get("blob")?;
                Ok(item)
            },
        )
        .optional()?;
    match item {
        Some(item) => Ok(Some(from_blob::<Value>(item)?.0)),
        None => Ok(None),
    }
}

pub fn get_all(txn: &Transaction<'_>) -> StateQueryResult<Vec<(AgentKvKey, AgentInfoSigned)>> {
    let mut stmt = txn.prepare(
        "
            SELECT key, blob FROM AgentInfo 
        ",
    )?;
    let items = stmt
        .query_and_then([], |row| {
            let key: Vec<u8> = row.get("key")?;
            let key: AgentKvKey = key.into();
            let item = row.get("blob")?;
            StateQueryResult::Ok((key.into(), from_blob::<Value>(item)?.0))
        })?
        .collect::<StateQueryResult<Vec<_>>>();

    items
}

pub fn get_all_values(txn: &Transaction<'_>) -> StateQueryResult<Vec<AgentInfoSigned>> {
    let mut stmt = txn.prepare(
        "
            SELECT blob FROM AgentInfo 
        ",
    )?;
    let items = stmt
        .query_and_then([], |row| {
            let item = row.get("blob")?;
            StateQueryResult::Ok(from_blob::<Value>(item)?.0)
        })?
        .collect::<StateQueryResult<Vec<_>>>();

    items
}

pub fn contains(txn: &Transaction<'_>, key: AgentKvKey) -> StateQueryResult<bool> {
    Ok(txn.query_row(
        "EXISTS(SELECT 1 FROM AgentInfo WHERE key = :key)",
        named_params! {
            ":key": key
        },
        |row| row.get(0),
    )?)
}

pub fn put(txn: &mut Transaction, info: AgentInfoSigned) -> StateMutationResult<()> {
    let key: AgentKvKey = (&info).try_into()?;
    let info = Value(info);
    mutations::insert_agent_info(txn, key, info.try_into()?)
}

pub fn delete(txn: &mut Transaction, key: AgentKvKey) -> StateMutationResult<()> {
    txn.execute(
        "DELETE FROM AgentInfo WHERE key = :key",
        named_params! {
            ":key": key
        },
    )?;
    Ok(())
}

pub fn get_agent_info(
    txn: &Transaction<'_>,
    space: DnaHash,
    agent: AgentPubKey,
) -> StateQueryResult<Option<AgentInfoSigned>> {
    let key: AgentKvKey = (space, agent).into();
    get(txn, key)
}
