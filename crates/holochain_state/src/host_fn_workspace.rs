use std::sync::Arc;

use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;

use crate::prelude::*;

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

    /// Some zome calls need to know the chain head, and we can't do async
    /// operations in zome calls, so this needs to be awkwardly tacked on here.
    /// The outer Option is for whether the head was precomputed at all,
    /// and the inner Option signifies whether the chain is empty or not.
    precomputed_chain_head: Option<Option<HeadInfo>>,
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
        Self::new_inner(authored, dht, cache, source_chain, dna_def)
    }

    fn new_inner(
        authored: DbWrite<DbKindAuthored>,
        dht: DbWrite<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        source_chain: SourceChain,
        dna_def: Arc<DnaDef>,
    ) -> SourceChainResult<Self> {
        Ok(Self {
            inner: HostFnWorkspace {
                source_chain: Some(source_chain.clone()),
                authored: authored.into(),
                dht: dht.into(),
                dna_def,
                cache,
                precomputed_chain_head: None,
            },
            source_chain,
        })
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
            precomputed_chain_head: None,
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

    pub fn chain_head_precomputed(&self) -> Option<Option<HeadInfo>> {
        self.precomputed_chain_head.clone()
    }

    pub async fn precompute_chain_head(&mut self) -> SourceChainResult<()> {
        self.precomputed_chain_head = Some(
            self.source_chain
                .as_ref()
                .expect("source chain must be present in workspace")
                .chain_head()
                .await?
                .map(Into::into),
        );
        Ok(())
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
            precomputed_chain_head: workspace.precomputed_chain_head,
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
            precomputed_chain_head: workspace.inner.precomputed_chain_head,
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
