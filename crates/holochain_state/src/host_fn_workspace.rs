use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::ReadAccess;
use holochain_types::env::DbReadOnly;
use holochain_types::env::DbWrite;

use crate::prelude::SourceChain;
use crate::prelude::SourceChainError;
use crate::prelude::SourceChainResult;
use crate::scratch::SyncScratch;

#[derive(Clone)]
pub struct HostFnWorkspace<SourceChainDb = DbWrite<DbKindAuthored>> {
    source_chain: Option<SourceChain<SourceChainDb>>,
    authored: DbReadOnly<DbKindAuthored>,
    dht: DbReadOnly<DbKindDht>,
    cache: DbWrite<DbKindCache>,
}

#[derive(Clone, shrinkwraprs::Shrinkwrap)]
pub struct SourceChainWorkspace {
    #[shrinkwrap(main_field)]
    inner: HostFnWorkspace,
    source_chain: SourceChain,
}

pub struct HostFnStores {
    pub authored: DbReadOnly<DbKindAuthored>,
    pub dht: DbReadOnly<DbKindDht>,
    pub cache: DbWrite<DbKindCache>,
    pub scratch: Option<SyncScratch>,
}

pub type HostFnWorkspaceReadOnly = HostFnWorkspace<DbReadOnly<DbKindAuthored>>;

impl HostFnWorkspace {
    pub async fn flush(
        self,
        network: &(dyn HolochainP2pDnaT + Send + Sync),
    ) -> SourceChainResult<()> {
        if let Some(sc) = self.source_chain {
            sc.flush(network).await?;
        }
        Ok(())
    }
}

impl SourceChainWorkspace {
    pub async fn new(
        authored: DbWrite<DbKindAuthored>,
        dht: DbReadOnly<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: AgentPubKey,
    ) -> SourceChainResult<Self> {
        let source_chain = SourceChain::new(authored.clone(), keystore, author).await?;
        Ok(Self {
            inner: HostFnWorkspace {
                source_chain: Some(source_chain.clone()),
                authored: authored.into(),
                dht,
                cache,
            },
            source_chain,
        })
    }
}

impl<SourceChainDb> HostFnWorkspace<SourceChainDb>
where
    SourceChainDb: ReadAccess<DbKindAuthored>,
{
    pub async fn new(
        authored: SourceChainDb,
        dht: DbReadOnly<DbKindDht>,
        cache: DbWrite<DbKindCache>,
        keystore: MetaLairClient,
        author: Option<AgentPubKey>,
    ) -> SourceChainResult<Self> {
        let source_chain = match author {
            Some(author) => Some(SourceChain::new(authored.clone(), keystore, author).await?),
            None => None,
        };
        Ok(Self {
            source_chain,
            authored: authored.into(),
            dht,
            cache,
        })
    }
    pub fn source_chain(&self) -> &Option<SourceChain<SourceChainDb>> {
        &self.source_chain
    }

    pub fn stores(&self) -> HostFnStores {
        HostFnStores {
            authored: self.authored.clone().into(),
            dht: self.dht.clone(),
            cache: self.cache.clone(),
            scratch: self.source_chain.as_ref().map(|sc| sc.scratch()),
        }
    }

    pub fn databases(
        &self,
    ) -> (
        DbReadOnly<DbKindAuthored>,
        DbReadOnly<DbKindDht>,
        DbWrite<DbKindCache>,
    ) {
        (
            self.authored.clone().into(),
            self.dht.clone(),
            self.cache.clone(),
        )
    }
}

impl SourceChainWorkspace {
    pub fn source_chain(&self) -> &SourceChain {
        &self.source_chain
    }
}

impl From<HostFnWorkspace> for HostFnWorkspaceReadOnly {
    fn from(workspace: HostFnWorkspace) -> Self {
        Self {
            source_chain: workspace.source_chain.map(|sc| sc.into()),
            authored: workspace.authored,
            dht: workspace.dht,
            cache: workspace.cache,
        }
    }
}

impl From<SourceChainWorkspace> for HostFnWorkspace {
    fn from(workspace: SourceChainWorkspace) -> Self {
        workspace.inner
    }
}

impl From<SourceChainWorkspace> for HostFnWorkspaceReadOnly {
    fn from(workspace: SourceChainWorkspace) -> Self {
        Self {
            source_chain: Some(workspace.source_chain.into()),
            authored: workspace.inner.authored.into(),
            dht: workspace.inner.dht,
            cache: workspace.inner.cache,
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
