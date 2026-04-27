//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `CapClaim` table.

use super::super::inner::cap_claim;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::CapClaimRow;
use holo_hash::AgentPubKey;

impl TxWrite<Dht> {
    pub async fn insert_cap_claim(
        &mut self,
        author: &AgentPubKey,
        tag: &str,
        grantor: &AgentPubKey,
        secret: &[u8],
    ) -> sqlx::Result<()> {
        cap_claim::insert_cap_claim(self.conn_mut(), author, tag, grantor, secret).await
    }
}

impl TxRead<Dht> {
    pub async fn get_cap_claims_by_grantor(
        &mut self,
        author: AgentPubKey,
        grantor: AgentPubKey,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        cap_claim::get_cap_claims_by_grantor(self.conn_mut(), author, grantor).await
    }

    pub async fn get_cap_claims_by_tag(
        &mut self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        cap_claim::get_cap_claims_by_tag(self.conn_mut(), author, tag).await
    }
}
