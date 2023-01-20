use std::sync::Arc;

use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::ReadAccess;
use holochain_types::db::DbRead;
use holochain_types::db::DbWrite;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_zome_types::DnaDef;
use holochain_zome_types::SignedActionHashed;

use crate::prelude::SourceChain;
use crate::prelude::SourceChainError;
use crate::prelude::SourceChainResult;
use crate::scratch::SyncScratch;

#[derive(Clone)]
pub struct HostFnWorkspace<
    SourceChainDb = DbWrite<DbKindAuthored>,
    SourceChainDht = DbWrite<DbKindDht>,
> {
    source_chain: Option<SourceChain<SourceChainDb, SourceChainDht>>,
    authored: DbRead<DbKindAuthored>,
    dht: DbRead<DbKindDht>,
    cache: DbWrite<DbKindCache>,
    dna_def: Arc<DnaDef>,
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

impl HostFnWorkspace {
    pub async fn flush(
        self,
        network: &(dyn HolochainP2pDnaT + Send + Sync),
    ) -> SourceChainResult<Vec<SignedActionHashed>> {
        match self.source_chain {
            Some(sc) => sc.flush(network).await,
            None => Ok(Vec::with_capacity(0)),
        }
    }

    /// Get a reference to the host fn workspace's dna def.
    pub fn dna_def(&self) -> Arc<DnaDef> {
        self.dna_def.clone()
    }
}

impl SourceChainWorkspace {
    pub async fn new(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
        dna_def: Arc<DnaDef>,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::new(
            authored.clone(),
            dht.clone(),
            dht_db_cache.clone(),
            keystore,
            author,
        )
        .await?;
        Self::new_inner(authored, dht, cache, source_chain, dna_def, false).await
    }

    /// Create a source chain workspace where the root caller is the init callback.
    pub async fn init_as_root(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
        dna_def: Arc<DnaDef>,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::new(
            authored.clone(),
            dht.clone(),
            dht_db_cache.clone(),
            keystore,
            author,
        )
        .await?;
        Self::new_inner(authored, dht, cache, source_chain, dna_def, true).await
    }

    /// Create a source chain with a blank chain head.
    /// You probably don't want this.
    /// This type is only useful for when a source chain
    /// really needs to be constructed before genesis runs.
    pub async fn raw_empty(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
        dna_def: Arc<DnaDef>,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::raw_empty(
            authored.clone(),
            dht.clone(),
            dht_db_cache.clone(),
            keystore,
            author,
        )
        .await?;
        Self::new_inner(authored, dht, cache, source_chain, dna_def, false).await
    }

    async fn new_inner(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        source_chain: SourceChain,
        dna_def: Arc<DnaDef>,
        init_is_root: bool,
    ) -> SourceChainResult<Self> {
        Ok(Self {
            inner: HostFnWorkspace {
                source_chain: Some(source_chain.clone()),
                authored: authored.into(),
                dht: dht.into(),
                dna_def,
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
        dht_db_cache: DhtDbQueryCache,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: Option<AgentPubKey>,
        dna_def: Arc<DnaDef>,
    ) -> SourceChainResult<Self> {
        let source_chain = match author {
            Some(author) => Some(
                SourceChain::new(
                    authored.clone(),
                    dht.clone(),
                    dht_db_cache.clone(),
                    keystore,
                    author,
                )
                .await?,
            ),
            None => None,
        };
        Ok(Self {
            source_chain,
            authored: authored.into(),
            dht: dht.into(),
            cache,
            dna_def,
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
            dna_def: workspace.dna_def,
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
            dna_def: workspace.inner.dna_def,
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
