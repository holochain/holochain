//! p2p_agent_store sql logic

use crate::prelude::*;
use crate::sql::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::KitsuneAgent;
use rusqlite::*;
use std::sync::Arc;

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

            ":storage_start_1": &record.storage_start_1,
            ":storage_end_1": &record.storage_end_1,
            ":storage_start_2": &record.storage_start_2,
            ":storage_end_2": &record.storage_end_2,
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
        let mut stmt = self
            .prepare(sql_p2p_agent_store::SELECT)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        Ok(stmt
            .query_row(named_params! { ":agent": &agent.0 }, |r| {
                let r = r.get_ref(0)?;
                let r = r.as_blob()?;
                let signed = AgentInfoSigned::decode(r)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
                Ok(signed)
            })
            .optional()?)
    }

    fn p2p_list(&self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        let mut stmt = self
            .prepare(sql_p2p_agent_store::SELECT_ALL)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut out = Vec::new();
        for r in stmt.query_map([], |r| {
            let r = r.get_ref(0)?;
            let r = r.as_blob()?;
            let signed = AgentInfoSigned::decode(r)
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

        let (storage_1, storage_2) = split_arc(&within_arc);

        let mut out = Vec::new();
        for r in stmt.query_map(
            named_params! {
                ":since_ms": clamp64(since_ms),
                ":until_ms": clamp64(until_ms),
                ":storage_start_1": storage_1.map(|s| s.0),
                ":storage_end_1": storage_1.map(|s| s.1),
                ":storage_start_2": storage_2.map(|s| s.0),
                ":storage_end_2": storage_2.map(|s| s.1),
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
    agent: Arc<KitsuneAgent>,

    // encoded binary
    encoded: Box<[u8]>,

    // additional queryable fields
    signed_at_ms: i64,
    expires_at_ms: i64,
    storage_center_loc: u32,

    // generated fields
    storage_start_1: Option<u32>,
    storage_end_1: Option<u32>,
    storage_start_2: Option<u32>,
    storage_end_2: Option<u32>,
}

pub type SplitRange = (u32, u32);
pub fn split_arc(arc: &DhtArc) -> (Option<SplitRange>, Option<SplitRange>) {
    let mut storage_1 = None;
    let mut storage_2 = None;

    use std::ops::{Bound, RangeBounds};
    let r = arc.range();
    let s = r.start_bound();
    let e = r.end_bound();
    match (s, e) {
        // in the zero length case, DhtArc returns two excluded bounds
        (Bound::Excluded(_), Bound::Excluded(_)) => (),
        // the only other case for DhtArc is two included bounds
        (Bound::Included(s), Bound::Included(e)) => {
            if s > e {
                storage_1 = Some((u32::MIN, *e));
                storage_2 = Some((*s, u32::MAX));
            } else {
                storage_1 = Some((*s, *e));
            }
        }
        // no other cases currently exist
        _ => unreachable!(),
    }

    (storage_1, storage_2)
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
        let agent = signed.agent.clone();

        let encoded = signed.encode().map_err(|e| anyhow::anyhow!(e))?;

        let signed_at_ms = signed.signed_at_ms;
        let expires_at_ms = signed.expires_at_ms;
        let arc = signed.storage_arc;

        let storage_center_loc = arc.center_loc.into();

        let (storage_1, storage_2) = split_arc(&arc);

        Ok(Self {
            agent,

            encoded,

            signed_at_ms: clamp64(signed_at_ms),
            expires_at_ms: clamp64(expires_at_ms),
            storage_center_loc,

            storage_start_1: storage_1.map(|s| s.0),
            storage_end_1: storage_1.map(|s| s.1),
            storage_start_2: storage_2.map(|s| s.0),
            storage_end_2: storage_2.map(|s| s.1),
        })
    }
}

#[cfg(test)]
mod p2p_test;
