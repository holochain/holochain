//! p2p_store sql logic

use crate::prelude::*;
use crate::schema::*;
use kitsune_p2p::agent_store::{AgentInfo, AgentInfoSigned};
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::KitsuneAgent;
use rusqlite::*;

/// Extension trait to treat connection instances
/// as p2p store accessors.
pub trait AsP2pConExt {
    /// Put an AgentInfoSigned record into the p2p_store
    fn p2p_put(&mut self, signed: &AgentInfoSigned) -> DatabaseResult<()>;

    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get(&mut self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// List all AgentInfoSigned records within a space in the p2p_store
    fn p2p_list(&mut self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agent list for gossip
    fn p2p_gossip_query(
        &mut self,
        since_ms: u64,
        until_ms: u64,
        within_arc: DhtArc,
    ) -> DatabaseResult<Vec<KitsuneAgent>>;

    /// Prune all expired AgentInfoSigned records from the p2p_store
    fn p2p_prune(&mut self) -> DatabaseResult<()>;
}

/// Extension trait to treat transaction instances
/// as p2p store accessors.
pub trait AsP2pTxExt {
    /// Put an AgentInfoSigned record into the p2p_store
    fn p2p_put(&self, signed: &AgentInfoSigned) -> DatabaseResult<()>;

    /// Get an AgentInfoSigned record from the p2p_store
    fn p2p_get(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>>;

    /// List all AgentInfoSigned records within a space in the p2p_store
    fn p2p_list(&self) -> DatabaseResult<Vec<AgentInfoSigned>>;

    /// Query agent list for gossip
    fn p2p_gossip_query(
        &self,
        since_ms: u64,
        until_ms: u64,
        within_arc: DhtArc,
    ) -> DatabaseResult<Vec<KitsuneAgent>>;

    /// Prune all expired AgentInfoSigned records from the p2p_store
    fn p2p_prune(&self) -> DatabaseResult<()>;
}

impl AsP2pConExt for crate::db::PConn {
    fn p2p_put(&mut self, signed: &AgentInfoSigned) -> DatabaseResult<()> {
        self.with_commit(move |writer| writer.p2p_put(signed))
    }

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

    fn p2p_prune(&mut self) -> DatabaseResult<()> {
        self.with_commit(move |writer| writer.p2p_prune())
    }
}

impl AsP2pTxExt for Transaction<'_> {
    fn p2p_put(&self, signed: &AgentInfoSigned) -> DatabaseResult<()> {
        let record = P2pRecord::from_signed(signed)?;
        self.execute(
            P2P_INSERT,
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

    fn p2p_get(&self, agent: &KitsuneAgent) -> DatabaseResult<Option<AgentInfoSigned>> {
        use std::convert::TryFrom;

        let mut stmt = self
            .prepare(P2P_SELECT)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        Ok(stmt
            .query_row(named_params! { ":agent": &agent.0 }, |r| {
                let encoded: Vec<u8> = r.get(0)?;
                let signed = AgentInfoSigned::try_from(encoded.as_ref())
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
                Ok(signed)
            })
            .optional()?)
    }

    fn p2p_list(&self) -> DatabaseResult<Vec<AgentInfoSigned>> {
        use std::convert::TryFrom;

        let mut stmt = self
            .prepare(P2P_SELECT_ALL)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut out = Vec::new();
        for r in stmt.query_map([], |r| {
            let encoded: Vec<u8> = r.get(0)?;
            let signed = AgentInfoSigned::try_from(encoded.as_ref())
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
            .prepare(P2P_GOSSIP_QUERY)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        let (storage_start_1, storage_end_1, storage_start_2, storage_end_2) =
            split_arc(&within_arc);

        let mut out = Vec::new();
        for r in stmt.query_map(
            named_params! {
                ":since_ms": &since_ms,
                ":until_ms": &until_ms,
                ":storage_start_1": &storage_start_1,
                ":storage_end_1": &storage_end_1,
                ":storage_start_2": &storage_start_2,
                ":storage_end_2": &storage_end_2,
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

    fn p2p_prune(&self) -> DatabaseResult<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        self.execute(P2P_PRUNE, named_params! { ":now": now })?;

        Ok(())
    }
}

/// Owned data dealing with a full p2p_store record.
#[derive(Debug)]
struct P2pRecord {
    agent: KitsuneAgent,

    // encoded binary
    encoded: Vec<u8>,

    // additional queryable fields
    signed_at_ms: u64,
    expires_at_ms: u64,
    storage_center_loc: u32,

    // generated fields
    storage_start_1: Option<u32>,
    storage_end_1: Option<u32>,
    storage_start_2: Option<u32>,
    storage_end_2: Option<u32>,
}

fn split_arc(arc: &DhtArc) -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
    let mut storage_start_1 = None;
    let mut storage_end_1 = None;
    let mut storage_start_2 = None;
    let mut storage_end_2 = None;

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
                storage_start_1 = Some(u32::MIN);
                storage_end_1 = Some(*e);
                storage_start_2 = Some(*s);
                storage_end_2 = Some(u32::MAX);
            } else {
                storage_start_1 = Some(*s);
                storage_end_1 = Some(*e);
            }
        }
        // no other cases currently exist
        _ => unreachable!(),
    }

    (
        storage_start_1,
        storage_end_1,
        storage_start_2,
        storage_end_2,
    )
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

        let (storage_start_1, storage_end_1, storage_start_2, storage_end_2) = split_arc(&arc);

        Ok(Self {
            agent,

            encoded,

            signed_at_ms,
            expires_at_ms,
            storage_center_loc,

            storage_start_1,
            storage_end_1,
            storage_start_2,
            storage_end_2,
        })
    }
}

#[cfg(test)]
mod p2p_test;
