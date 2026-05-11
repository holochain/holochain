//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `CapClaim` table.

use super::super::inner::cap_claim;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::CapClaimRow;
use holo_hash::AgentPubKey;

impl DbWrite<Dht> {
    pub async fn insert_cap_claim(
        &self,
        author: &AgentPubKey,
        tag: &str,
        grantor: &AgentPubKey,
        secret: &[u8],
    ) -> sqlx::Result<()> {
        cap_claim::insert_cap_claim(self.pool(), author, tag, grantor, secret).await
    }
}

impl DbRead<Dht> {
    pub async fn get_cap_claims_by_grantor(
        &self,
        author: AgentPubKey,
        grantor: AgentPubKey,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        cap_claim::get_cap_claims_by_grantor(self.pool(), author, grantor).await
    }

    pub async fn get_cap_claims_by_tag(
        &self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        cap_claim::get_cap_claims_by_tag(self.pool(), author, tag).await
    }
}
