use crate::prelude::*;
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct HostFnWorkspace<
    SourceChainDb = DbWrite<DbKindAuthored>,
    SourceChainDht = DbWrite<DbKindDht>,
> {
    source_chain: Option<SourceChain<SourceChainDb, SourceChainDht>>,
    authored: DbRead<DbKindAuthored>,
    dht: DbRead<DbKindDht>,
    cache: DbWrite<DbKindCache>,
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
    pub authored: DbRead<DbKindAuthored>,
    pub dht: DbRead<DbKindDht>,
    pub cache: DbWrite<DbKindCache>,
    pub scratch: Option<SyncScratch>,
}

pub type HostFnWorkspaceRead = HostFnWorkspace<DbRead<DbKindAuthored>, DbRead<DbKindDht>>;

impl SourceChainWorkspace {
    pub async fn new(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain =
            SourceChain::new(authored.clone(), dht.clone(), keystore, author).await?;
        Self::new_inner(authored, dht, cache, source_chain, false)
    }

    /// Create a source chain workspace where the root caller is the init callback.
    pub async fn init_as_root(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain =
            SourceChain::new(authored.clone(), dht.clone(), keystore, author).await?;
        Self::new_inner(authored, dht, cache, source_chain, true)
    }

    /// Create a source chain with a blank chain head.
    /// You probably don't want this.
    /// This type is only useful for when a source chain
    /// really needs to be constructed before genesis runs.
    pub async fn raw_empty(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain =
            SourceChain::raw_empty(authored.clone(), dht.clone(), keystore, author).await?;
        Self::new_inner(authored, dht, cache, source_chain, false)
    }

    fn new_inner(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        source_chain: SourceChain,
        init_is_root: bool,
    ) -> SourceChainResult<Self> {
        Ok(Self {
            inner: HostFnWorkspace {
                source_chain: Some(source_chain.clone()),
                authored: authored.into(),
                dht: dht.into(),
                cache,
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

impl<SourceChainDb, SourceChainDht> HostFnWorkspace<SourceChainDb, SourceChainDht>
where
    SourceChainDb: ReadAccess<DbKindAuthored>,
    SourceChainDht: ReadAccess<DbKindDht>,
{
    pub async fn new(
        authored: SourceChainDb,
        dht: SourceChainDht,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: Option<AgentPubKey>,
    ) -> SourceChainResult<Self> {
        let source_chain = match author {
            Some(author) => {
                Some(SourceChain::new(authored.clone(), dht.clone(), keystore, author).await?)
            }
            None => None,
        };
        Ok(Self {
            source_chain,
            authored: authored.into(),
            dht: dht.into(),
            cache,
            init_is_root: false,
        })
    }

    pub fn source_chain(&self) -> &Option<SourceChain<SourceChainDb, SourceChainDht>> {
        &self.source_chain
    }

    pub fn author(&self) -> Option<Arc<AgentPubKey>> {
        self.source_chain.as_ref().map(|s| s.to_agent_pubkey())
    }

    pub fn stores(&self) -> HostFnStores {
        HostFnStores {
            authored: self.authored.clone(),
            dht: self.dht.clone(),
            cache: self.cache.clone(),
            scratch: self.source_chain.as_ref().map(|sc| sc.scratch()),
        }
    }

    pub fn databases(
        &self,
    ) -> (
        DbRead<DbKindAuthored>,
        DbRead<DbKindDht>,
        DbWrite<DbKindCache>,
    ) {
        (self.authored.clone(), self.dht.clone(), self.cache.clone())
    }
}

impl SourceChainWorkspace {
    pub fn source_chain(&self) -> &SourceChain {
        &self.source_chain
    }
}

impl From<HostFnWorkspace> for HostFnWorkspaceRead {
    fn from(workspace: HostFnWorkspace) -> Self {
        Self {
            source_chain: workspace.source_chain.map(|sc| sc.into()),
            authored: workspace.authored,
            dht: workspace.dht,
            cache: workspace.cache,
            init_is_root: workspace.init_is_root,
        }
    }
}

impl From<SourceChainWorkspace> for HostFnWorkspace {
    fn from(workspace: SourceChainWorkspace) -> Self {
        workspace.inner
    }
}

impl From<SourceChainWorkspace> for HostFnWorkspaceRead {
    fn from(workspace: SourceChainWorkspace) -> Self {
        Self {
            source_chain: Some(workspace.source_chain.into()),
            authored: workspace.inner.authored,
            dht: workspace.inner.dht,
            cache: workspace.inner.cache,
            init_is_root: workspace.inner.init_is_root,
        }
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
