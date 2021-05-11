//! p2p_store sql logic

use crate::prelude::*;
use crate::schema::*;
use kitsune_p2p::agent_store::{AgentInfo, AgentInfoSigned};
use kitsune_p2p::{KitsuneAgent, KitsuneSpace};
use rusqlite::*;

#[cfg(test)]
mod p2p_test;

/// Owned data dealing with a full p2p_store record.
#[derive(Debug)]
pub struct P2pRecordOwned {
    // primary key items
    space: KitsuneSpace,
    agent: KitsuneAgent,
    signed_at_ms: u64,

    // encoded binary
    encoded: Vec<u8>,

    // additional queryable fields
    expires_at_ms: u64,
    storage_center_loc: u32,
    storage_half_length: u32,
    storage_start_1: Option<u32>,
    storage_end_1: Option<u32>,
    storage_start_2: Option<u32>,
    storage_end_2: Option<u32>,
}

impl P2pRecordOwned {
    pub fn from_signed(signed: &AgentInfoSigned) -> DatabaseResult<Self> {
        use std::convert::TryFrom;

        let info = AgentInfo::try_from(signed).map_err(|e| anyhow::anyhow!(e))?;
        let space = info.as_space_ref().clone();
        let agent = info.as_agent_ref().clone();
        let signed_at_ms = info.signed_at_ms();

        let encoded = <Vec<u8>>::try_from(signed).map_err(|e| anyhow::anyhow!(e))?;

        let expires_at_ms = signed_at_ms + info.expires_after_ms();
        let arc = info.dht_arc().map_err(|e| anyhow::anyhow!(e))?;
        let storage_center_loc = arc.center_loc.into();
        let storage_half_length = arc.half_length;
        let storage_start_1 = None;
        let storage_end_1 = None;
        let storage_start_2 = None;
        let storage_end_2 = None;

        Ok(Self {
            space,
            agent,
            signed_at_ms,

            encoded,

            expires_at_ms,
            storage_center_loc,
            storage_half_length,
            storage_start_1,
            storage_end_1,
            storage_start_2,
            storage_end_2,
        })
    }
}

/// Extension trait to treat rusqlite Transaction instances
/// as p2p store accessors.
pub trait TxAsP2pExt {
    fn p2p_insert(&self, signed: &AgentInfoSigned) -> DatabaseResult<()>;

    fn p2p_select_all(&self, space: &KitsuneSpace) -> DatabaseResult<Vec<AgentInfoSigned>>;

    fn prune(&self, expires_at_ms: u64) -> DatabaseResult<()>;
}

impl TxAsP2pExt for Transaction<'_> {
    fn p2p_insert(&self, signed: &AgentInfoSigned) -> DatabaseResult<()> {
        let record = P2pRecordOwned::from_signed(signed)?;
        self.execute(
            P2P_INSERT,
            named_params! {
                ":space": &record.space.0,
                ":agent": &record.agent.0,
                ":signed_at_ms": &record.signed_at_ms,

                ":encoded": &record.encoded,

                ":expires_at_ms": &record.expires_at_ms,
                ":storage_center_loc": &record.storage_center_loc,
                ":storage_half_length": &record.storage_half_length,
                ":storage_start_1": &record.storage_start_1,
                ":storage_end_1": &record.storage_end_1,
                ":storage_start_2": &record.storage_start_2,
                ":storage_end_2": &record.storage_end_2,
            },
        )?;
        Ok(())
    }

    fn p2p_select_all(&self, space: &KitsuneSpace) -> DatabaseResult<Vec<AgentInfoSigned>> {
        use std::convert::TryFrom;

        let mut stmt = self
            .prepare(P2P_SELECT_ALL)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let mut out = Vec::new();
        for r in stmt.query_map(named_params! { ":space": &space.0 }, |r| {
            let encoded: Vec<u8> = r.get(0)?;
            let signed = AgentInfoSigned::try_from(encoded.as_ref())
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

            Ok(signed)
        })? {
            out.push(r?);
        }
        Ok(out)
    }

    fn prune(&self, expires_at_ms: u64) -> DatabaseResult<()> {
        self.execute(
            P2P_PRUNE,
            named_params! {
                ":expires_at_ms": expires_at_ms,
            },
        )?;
        Ok(())
    }
}
