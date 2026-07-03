use crate::prelude::*;
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct HostFnWorkspace {
    source_chain: Option<SourceChain>,
    dht_store: DhtStore,
    /// Did the root call that started this call chain
    /// come from an init callback.
    /// This is needed so that we don't run init recursively inside
    /// init calls.
    init_is_root: bool,
}

#[derive(Clone, shrinkwraprs::Shrinkwrap)]
pub struct SourceChainWorkspace {
    #[shrinkwrap(main_field)]
    inner: HostFnWorkspace,
    source_chain: SourceChain,
}

pub struct HostFnStores {
    pub scratch: Option<SyncScratch>,
    pub dht_store: Option<DhtStore>,
}

pub type HostFnWorkspaceRead = HostFnWorkspace;

impl SourceChainWorkspace {
    pub async fn new(
        dht_store: DhtStore,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::new(dht_store.clone(), keystore, author).await?;
        Self::new_inner(dht_store, source_chain, false)
    }

    /// Create a source chain workspace where the root caller is the init callback.
    pub async fn init_as_root(
        dht_store: DhtStore,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::new(dht_store.clone(), keystore, author).await?;
        Self::new_inner(dht_store, source_chain, true)
    }

    /// Create a source chain with a blank chain head.
    /// You probably don't want this.
    /// This type is only useful for when a source chain
    /// really needs to be constructed before genesis runs.
    pub async fn raw_empty(
        dht_store: DhtStore,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::raw_empty(dht_store.clone(), keystore, author).await?;
        Self::new_inner(dht_store, source_chain, false)
    }

    fn new_inner(
        dht_store: DhtStore,
        source_chain: SourceChain,
        init_is_root: bool,
    ) -> SourceChainResult<Self> {
        Ok(Self {
            inner: HostFnWorkspace {
                source_chain: Some(source_chain.clone()),
                dht_store,
                init_is_root,
            },
            source_chain,
        })
    }

    /// Did this zome call chain originate from within
    /// an init callback.
    pub fn called_from_init(&self) -> bool {
        self.inner.init_is_root
    }
}

impl HostFnWorkspace {
    pub async fn new(
        dht_store: DhtStore,
        keystore: MetaLairClient,
        author: Option<AgentPubKey>,
    ) -> SourceChainResult<Self> {
        let source_chain = match author {
            Some(author) => Some(SourceChain::new(dht_store.clone(), keystore, author).await?),
            None => None,
        };
        Ok(Self {
            source_chain,
            dht_store,
            init_is_root: false,
        })
    }

    pub fn source_chain(&self) -> &Option<SourceChain> {
        &self.source_chain
    }

    pub fn author(&self) -> Option<Arc<AgentPubKey>> {
        self.source_chain.as_ref().map(|s| s.to_agent_pubkey())
    }

    pub fn stores(&self) -> HostFnStores {
        HostFnStores {
            scratch: self.source_chain.as_ref().map(|sc| sc.scratch()),
            dht_store: Some(self.dht_store.clone()),
        }
    }
}

impl SourceChainWorkspace {
    pub fn source_chain(&self) -> &SourceChain {
        &self.source_chain
    }
}

impl From<SourceChainWorkspace> for HostFnWorkspace {
    fn from(workspace: SourceChainWorkspace) -> Self {
        workspace.inner
    }
}

impl std::convert::TryFrom<HostFnWorkspace> for SourceChainWorkspace {
    type Error = SourceChainError;

    fn try_from(value: HostFnWorkspace) -> Result<Self, Self::Error> {
        let sc = match value.source_chain.clone() {
            Some(sc) => sc,
            None => return Err(SourceChainError::SourceChainMissing),
        };
        Ok(Self {
            inner: value,
            source_chain: sc,
        })
    }
}
