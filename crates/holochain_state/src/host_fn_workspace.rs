use std::sync::Arc;

use holo_hash::AgentPubKey;
use holochain_types::env::EnvRead;
use holochain_types::env::EnvWrite;

use crate::prelude::SourceChainResult;
use crate::prelude::StateMutationResult;
use crate::scratch::Scratch;

#[derive(Clone)]
pub struct HostFnWorkspace {
    source_chain: crate::source_chain2::SourceChain,
    vault: EnvWrite,
    cache: EnvWrite,
}

pub struct HostFnStores {
    pub vault: EnvRead,
    pub cache: EnvWrite,
    pub scratch: Arc<Scratch>,
}
pub type Vault = EnvRead;
pub type Cache = EnvWrite;

impl HostFnWorkspace {
    pub fn new(vault: EnvWrite, cache: EnvWrite, author: AgentPubKey) -> SourceChainResult<Self> {
        let source_chain = crate::source_chain2::SourceChain::new(vault.clone().into(), author)?;
        Ok(Self {
            source_chain,
            vault,
            cache,
        })
    }

    pub fn flush(self) -> SourceChainResult<()> {
        self.source_chain.flush()
    }

    pub fn source_chain(&self) -> &crate::source_chain2::SourceChain {
        &self.source_chain
    }

    pub fn stores(&self) -> SourceChainResult<HostFnStores> {
        Ok(HostFnStores {
            vault: self.vault.clone().into(),
            cache: self.cache.clone(),
            scratch: Arc::new(self.source_chain.snapshot()?),
        })
    }

    pub fn databases(&self) -> (Vault, Cache) {
        (self.vault.clone().into(), self.cache.clone())
    }
}
