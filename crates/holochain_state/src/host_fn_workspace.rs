use std::sync::Arc;

use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::ReadAccess;
use holochain_types::env::DbRead;
use holochain_types::env::DbWrite;
use holochain_zome_types::SignedHeaderHashed;

use crate::prelude::SourceChain;
use crate::prelude::SourceChainError;
use crate::prelude::SourceChainResult;
use crate::scratch::SyncScratch;
use holochain_zome_types::Zome;

#[derive(Clone)]
pub struct HostFnWorkspace<
    SourceChainDb = DbWrite<DbKindAuthored>,
    SourceChainDht = DbWrite<DbKindDht>,
> {
    source_chain: Option<SourceChain<SourceChainDb, SourceChainDht>>,
    authored: DbRead<DbKindAuthored>,
    dht: DbRead<DbKindDht>,
    cache: DbWrite<DbKindCache>,
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
    ) -> SourceChainResult<Vec<(Option<Zome>, SignedHeaderHashed)>> {
        match self.source_chain {
            Some(sc) => sc.flush(network).await,
            None => Ok(Vec::with_capacity(0)),
        }
    }
}

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
        Ok(Self {
            inner: HostFnWorkspace {
                source_chain: Some(source_chain.clone()),
                authored: authored.into(),
                dht: dht.clone().into(),
                cache,
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
