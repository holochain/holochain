//! p2p_store sql logic

use crate::schema::*;
use crate::prelude::*;
use holo_hash::{DnaHash, AgentPubKey};
use rusqlite::*;

#[cfg(test)]
mod p2p_test;

/// Reference to data dealing with a full p2p_store record.
pub struct P2pRecordRef<'lt> {
    space: &'lt DnaHash,
    agent: &'lt AgentPubKey,
    signed_at_ms: u64,
    expires_at_ms: u64,
    encoded: &'lt [u8],
    storage_center_loc: u32,
    storage_half_length: u32,
    storage_start_1: Option<u32>,
    storage_end_1: Option<u32>,
    storage_start_2: Option<u32>,
    storage_end_2: Option<u32>,
}

/// Owned data dealing with a full p2p_store record.
#[allow(dead_code)]
#[derive(Debug)]
pub struct P2pRecordOwned {
    space: DnaHash,
    agent: AgentPubKey,
    signed_at_ms: u64,
    expires_at_ms: u64,
    encoded: Vec<u8>,
    storage_center_loc: u32,
    storage_half_length: u32,
    storage_start_1: Option<u32>,
    storage_end_1: Option<u32>,
    storage_start_2: Option<u32>,
    storage_end_2: Option<u32>,
}

/// Extension trait to treat rusqlite Transaction instances
/// as p2p store accessors.
pub trait TxAsP2pExt {
    fn p2p_insert(
        &self,
        record: P2pRecordRef<'_>,
    ) -> DatabaseResult<()>;

    fn p2p_select_all(&self, space: &DnaHash) -> DatabaseResult<Vec<P2pRecordOwned>>;

    fn prune(&self, expires_at_ms: u64) -> DatabaseResult<()>;
}

impl TxAsP2pExt for Transaction<'_> {
    fn p2p_insert(
        &self,
        record: P2pRecordRef<'_>,
    ) -> DatabaseResult<()> {
        self.execute(
            P2P_INSERT,
            named_params! {
                ":space": &record.space,
                ":agent": &record.agent,
                ":signed_at_ms": &record.signed_at_ms,
                ":expires_at_ms": &record.expires_at_ms,
                ":encoded": &record.encoded,
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

    fn p2p_select_all(&self, space: &DnaHash) -> DatabaseResult<Vec<P2pRecordOwned>> {
        let mut stmt = self.prepare(P2P_SELECT_ALL)?;
        let mut out = Vec::new();
        for r in stmt.query_map(named_params! { ":space": space }, |r| {
            let space: DnaHash = r.get(0)?;
            let agent: AgentPubKey = r.get(1)?;
            let signed_at_ms: u64 = r.get(2)?;
            let expires_at_ms: u64 = r.get(3)?;
            let encoded: Vec<u8> = r.get(4)?;
            let storage_center_loc: u32 = r.get(5)?;
            let storage_half_length: u32 = r.get(6)?;
            let storage_start_1: Option<u32> = r.get(7)?;
            let storage_end_1: Option<u32> = r.get(8)?;
            let storage_start_2: Option<u32> = r.get(9)?;
            let storage_end_2: Option<u32> = r.get(10)?;

            Ok(P2pRecordOwned {
                space,
                agent,
                signed_at_ms,
                expires_at_ms,
                encoded,
                storage_center_loc,
                storage_half_length,
                storage_start_1,
                storage_end_1,
                storage_start_2,
                storage_end_2,
            })
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
