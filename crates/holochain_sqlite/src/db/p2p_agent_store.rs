//! p2p_agent_store sql logic

use crate::prelude::*;
use crate::sql::*;
use kitsune_p2p::agent_store::{AgentInfo, AgentInfoSigned};
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::KitsuneAgent;
use rusqlite::*;

/// Extension trait to treat connection instances
/// as p2p store accessors.
pub trait AsP2pStateConExt {
    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// List all AgentInfoSigned records within a space in the p2p_agent_store
    fn p2p_list(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agent list for gossip
    fn p2p_gossip_query(
        &mut self,
        since_ms: u64,
        until_ms: u64,
        within_arc: DhtArc,
    ) -> DatabaseResult<Vec<KitsuneAgent>>;
}

/// Extension trait to treat transaction instances
/// as p2p store accessors.
pub trait AsP2pStateTxExt {
    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// List all AgentInfoSigned records within a space in the p2p_agent_store
    fn p2p_list(&self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agent list for gossip
    fn p2p_gossip_query(
        &self,
        since_ms: u64,
        until_ms: u64,
        within_arc: DhtArc,
    ) -> DatabaseResult<Vec<KitsuneAgent>>;
}

impl AsP2pStateConExt for crate::db::PConn {
    fn p2p_get(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_get(agent))
    }

    fn p2p_list(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        self.with_reader(move |reader| reader.p2p_list())
    }

    fn p2p_gossip_query(
        &mut self,
        since_ms: u64,
        until_ms: u64,
        within_arc: DhtArc,
    ) -> DatabaseResult<Vec<KitsuneAgent>> {
        self.with_reader(move |reader| reader.p2p_gossip_query(since_ms, until_ms, within_arc))
    }
}

/// Put an AgentInfoSigned record into the p2p_store
pub async fn p2p_put(db: &DbWrite, signed: &AgentInfoSigned) -> DatabaseResult<()> {
    let record = P2pRecord::from_signed(signed)?;
    db.async_commit(move |txn| tx_p2p_put(txn, record)).await
}

/// Put an iterator of AgentInfoSigned records into the p2p_store
pub async fn p2p_put_all(
    db: &DbWrite,
    signed: impl Iterator<Item = &AgentInfoSigned>,
) -> DatabaseResult<()> {
    let mut records = Vec::new();
    for s in signed {
        records.push(P2pRecord::from_signed(s)?);
    }
    db.async_commit(move |txn| {
        for record in records {
            tx_p2p_put(txn, record)?;
        }
        Ok(())
    })
    .await
}

fn tx_p2p_put(txn: &mut Transaction, record: P2pRecord) -> DatabaseResult<()> {
    txn.execute(
        sql_p2p_agent_store::INSERT,
        named_params! {
            ":agent": &record.agent.0,

            ":encoded": &record.encoded,

            ":signed_at_ms": &record.signed_at_ms,
            ":expires_at_ms": &record.expires_at_ms,
            ":storage_center_loc": &record.storage_center_loc,

            ":storage_start_loc": &record.storage_start_loc,
            ":storage_end_loc": &record.storage_end_loc,
        },
    )?;
    Ok(())
}

/// Prune all expired AgentInfoSigned records from the p2p_store
pub async fn p2p_prune(db: &DbWrite) -> DatabaseResult<()> {
    db.async_commit(move |txn| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        txn.execute(sql_p2p_agent_store::PRUNE, named_params! { ":now": now })?;
        DatabaseResult::Ok(())
    })
    .await?;

    Ok(())
}
impl AsP2pStateTxExt for Transaction<'_> {
    fn p2p_get(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        use std::convert::TryFrom;

        let mut stmt = self
            .prepare(sql_p2p_agent_store::SELECT)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        Ok(stmt
            .query_row(named_params! { ":agent": &agent.0 }, |r| {
                let r = r.get_ref(0)?;
                let r = r.as_blob()?;
                let signed = AgentInfoSigned::try_from(r)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
                Ok(signed)
            })
            .optional()?)
    }

    fn p2p_list(&self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        use std::convert::TryFrom;

        let mut stmt = self
            .prepare(sql_p2p_agent_store::SELECT_ALL)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut out = Vec::new();
        for r in stmt.query_map([], |r| {
            let r = r.get_ref(0)?;
            let r = r.as_blob()?;
            let signed = AgentInfoSigned::try_from(r)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

            Ok(signed)
        })? {
            out.push(r?);
        }
        Ok(out)
    }

    fn p2p_gossip_query(
        &self,
        since_ms: u64,
        until_ms: u64,
        within_arc: DhtArc,
    ) -> DatabaseResult<Vec<KitsuneAgent>> {
        let mut stmt = self
            .prepare(sql_p2p_agent_store::GOSSIP_QUERY)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        let (start, end) = within_arc.primitive_range_transposed();

        let mut out = Vec::new();
        for r in stmt.query_map(
            named_params! {
                ":since_ms": clamp64(since_ms),
                ":until_ms": clamp64(until_ms),
                ":storage_start_loc": start,
                ":storage_end_loc": end,
            },
            |r| {
                let agent: Vec<u8> = r.get(0)?;
                Ok(KitsuneAgent(agent))
            },
        )? {
            out.push(r?);
        }
        Ok(out)
    }
}

/// Owned data dealing with a full p2p_agent_store record.
#[derive(Debug)]
struct P2pRecord {
    agent: KitsuneAgent,

    // encoded binary
    encoded: Vec<u8>,

    // additional queryable fields
    signed_at_ms: i64,
    expires_at_ms: i64,
    storage_center_loc: u32,

    // generated fields
    storage_start_loc: Option<u32>,
    storage_end_loc: Option<u32>,
}

fn clamp64(u: u64) -> i64 {
    if u > i64::MAX as u64 {
        i64::MAX
    } else {
        u as i64
    }
}

impl P2pRecord {
    pub fn from_signed(signed: &AgentInfoSigned) -> DatabaseResult<Self> {
        use std::convert::TryFrom;

        let info = AgentInfo::try_from(signed).map_err(|e| anyhow::anyhow!(e))?;
        let agent = info.as_agent_ref().clone();

        let encoded = <Vec<u8>>::try_from(signed).map_err(|e| anyhow::anyhow!(e))?;

        let signed_at_ms = info.signed_at_ms();
        let expires_at_ms = signed_at_ms + info.expires_after_ms();
        let arc = info.dht_arc().map_err(|e| anyhow::anyhow!(e))?;

        let storage_center_loc = arc.center_loc.into();

        let (storage_start_loc, storage_end_loc) = arc.primitive_range_transposed();

        Ok(Self {
            agent,

            encoded,

            signed_at_ms: clamp64(signed_at_ms),
            expires_at_ms: clamp64(expires_at_ms),
            storage_center_loc,

            storage_start_loc,
            storage_end_loc,
        })
    }
}

#[cfg(test)]
mod p2p_test;
