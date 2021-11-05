use holo_hash::AgentPubKey;
use holochain_p2p::HolochainP2pDnaT;
use holochain_types::env::EnvRead;
use holochain_types::env::EnvWrite;
use holochain_zome_types::SignedHeaderHashed;

use crate::prelude::SourceChain;
use crate::prelude::SourceChainResult;
use crate::scratch::SyncScratch;
use holochain_zome_types::Zome;

#[derive(Clone)]
pub struct HostFnWorkspace {
    source_chain: SourceChain,
    vault: EnvWrite,
    cache: EnvWrite,
}

pub struct HostFnStores {
    pub vault: EnvRead,
    pub cache: EnvWrite,
    pub scratch: SyncScratch,
}
pub type Vault = EnvRead;
pub type Cache = EnvWrite;

impl HostFnWorkspace {
    pub async fn new(
        vault: EnvWrite,
        cache: EnvWrite,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::new(vault.clone(), author).await?;
        Ok(Self {
            source_chain,
            vault,
            cache,
        })
    }

    pub async fn flush(
        self,
        network: &(dyn HolochainP2pDnaT + Send + Sync),
    ) -> SourceChainResult<Vec<(Option<Zome>, SignedHeaderHashed)>> {
        self.source_chain.flush(network).await
    }

    pub fn source_chain(&self) -> &SourceChain {
        &self.source_chain
    }

    pub fn stores(&self) -> HostFnStores {
        HostFnStores {
            vault: self.vault.clone().into(),
            cache: self.cache.clone(),
            scratch: self.source_chain.scratch(),
        }
    }

    pub fn databases(&self) -> (Vault, Cache) {
        (self.vault.clone().into(), self.cache.clone())
    }
}
