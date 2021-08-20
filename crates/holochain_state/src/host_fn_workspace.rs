use holo_hash::AgentPubKey;
use holochain_types::env::EnvRead;
use holochain_types::env::EnvWrite;

use crate::prelude::SourceChain;
use crate::prelude::SourceChainResult;
use crate::scratch::SyncScratch;

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

    pub async fn flush(self) -> SourceChainResult<()> {
        self.source_chain.flush().await
    }

    pub fn source_chain(&self) -> &SourceChain {
        &self.source_chain
    }

    pub fn source_chain_mut(&mut self) -> &mut SourceChain {
        &mut self.source_chain
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
