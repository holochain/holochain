//! in-memory persistence module for kitsune direct

use crate::types::persist::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use kitsune_p2p::dht::spacetime::Topology;
use kitsune_p2p::dht_arc::{DhtArcSet, DhtLocation};
use kitsune_p2p::event::TimeWindow;
use kitsune_p2p_types::dht::PeerStrat;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// construct a new in-memory persistence module for kitsune direct
pub fn new_persist_mem() -> KdPersist {
    KdPersist(PersistMem::new())
}

// -- private -- //

struct AgentStoreInner {
    pub_key_to_info_map: HashMap<KdHash, KdAgentInfo>,
}

struct AgentStore(Share<AgentStoreInner>);

impl AgentStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Share::new(AgentStoreInner {
            pub_key_to_info_map: HashMap::new(),
        })))
    }

    pub fn insert(&self, agent_info: KdAgentInfo) -> KdResult<()> {
        let agent = agent_info.agent().clone();
        self.0
            .share_mut(move |i, _| {
                match i.pub_key_to_info_map.entry(agent) {
                    Entry::Occupied(mut e) => {
                        if e.get().signed_at_ms() < agent_info.signed_at_ms() {
                            e.insert(agent_info);
                        }
                    }
                    Entry::Vacant(e) => {
                        e.insert(agent_info);
                    }
                }

                Ok(())
            })
            .map_err(KdError::other)
    }

    pub fn get(&self, agent: &KdHash) -> KdResult<KdAgentInfo> {
        self.0
            .share_mut(move |i, _| match i.pub_key_to_info_map.get(agent) {
                Some(agent_info) => Ok(agent_info.clone()),
                None => Err("agent not found".into()),
            })
            .map_err(KdError::other)
    }

    pub fn get_all(&self) -> KdResult<Vec<KdAgentInfo>> {
        self.0
            .share_mut(move |i, _| Ok(i.pub_key_to_info_map.values().cloned().collect()))
            .map_err(KdError::other)
    }
}

struct EntryStoreInner {
    hash_to_entry_map: HashMap<KdHash, KdEntrySigned>,
}

struct EntryStore(Share<EntryStoreInner>);

impl EntryStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Share::new(EntryStoreInner {
            hash_to_entry_map: HashMap::new(),
        })))
    }

    pub fn insert(&self, entry_signed: KdEntrySigned) -> KdResult<()> {
        let hash = entry_signed.hash().clone();
        self.0
            .share_mut(move |i, _| {
                i.hash_to_entry_map.insert(hash, entry_signed);

                Ok(())
            })
            .map_err(KdError::other)
    }

    pub fn get(&self, hash: &KdHash) -> KdResult<KdEntrySigned> {
        self.0
            .share_mut(move |i, _| match i.hash_to_entry_map.get(hash) {
                Some(entry_signed) => Ok(entry_signed.clone()),
                None => Err("hash not found".into()),
            })
            .map_err(KdError::other)
    }

    pub fn get_all(&self) -> KdResult<Vec<KdEntrySigned>> {
        self.0
            .share_mut(move |i, _| Ok(i.hash_to_entry_map.values().cloned().collect()))
            .map_err(KdError::other)
    }
}

struct AgentEntryStoreInner {
    agent_to_entry_store_map: HashMap<KdHash, Arc<EntryStore>>,
}

struct AgentEntryStore(Share<AgentEntryStoreInner>);

impl AgentEntryStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Share::new(AgentEntryStoreInner {
            agent_to_entry_store_map: HashMap::new(),
        })))
    }

    pub fn get(&self, agent: &KdHash) -> KdResult<Arc<EntryStore>> {
        self.0
            .share_mut(|i, _| match i.agent_to_entry_store_map.get(agent) {
                None => Err("agent not found".into()),
                Some(entry_store) => Ok(entry_store.clone()),
            })
            .map_err(KdError::other)
    }

    pub fn get_mut(&self, agent: KdHash) -> KdResult<Arc<EntryStore>> {
        self.0
            .share_mut(move |i, _| {
                Ok(i.agent_to_entry_store_map
                    .entry(agent)
                    .or_insert_with(EntryStore::new)
                    .clone())
            })
            .map_err(KdError::other)
    }
}

struct UiEntry {
    mime: String,
    data: Box<[u8]>,
}

struct UiStoreInner {
    uri_to_file_map: HashMap<String, Arc<UiEntry>>,
}

struct UiStore(Share<UiStoreInner>);

impl UiStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Share::new(UiStoreInner {
            uri_to_file_map: HashMap::new(),
        })))
    }

    pub fn check_add(&self, root: &KdHash, entry: &KdEntrySigned) -> KdResult<()> {
        use kitsune_p2p_direct_api::kd_sys_kind::*;

        if entry.kind() == "s.file" {
            return match KdSysKind::from_kind(entry.kind(), entry.raw_data().clone()) {
                Ok(KdSysKind::File(file)) => {
                    let path = format!("/{}/{}", root, file.name);
                    println!("caching ui file: {}", path);
                    let data = entry.as_binary_ref().to_vec().into_boxed_slice();
                    self.0
                        .share_mut(|i, _| {
                            i.uri_to_file_map.insert(
                                path,
                                Arc::new(UiEntry {
                                    mime: file.mime.clone(),
                                    data,
                                }),
                            );
                            Ok(())
                        })
                        .map_err(KdError::other)
                }
                oth => Err(format!("UNEXPECTED: {:?}", oth).into()),
            };
        }

        Ok(())
    }

    pub fn get(&self, path: &str) -> KdResult<Arc<UiEntry>> {
        self.0
            .share_mut(|i, _| match i.uri_to_file_map.get(path) {
                None => Err(format!("404: {}", path).into()),
                Some(ui_entry) => Ok(ui_entry.clone()),
            })
            .map_err(KdError::other)
    }
}

struct PersistMemInner {
    tls: Option<TlsConfig>,
    priv_keys: HashMap<KdHash, sodoken::BufReadSized<64>>,
    agent_info: HashMap<KdHash, Arc<AgentStore>>,
    entries: HashMap<KdHash, Arc<AgentEntryStore>>,
    ui_cache: Arc<UiStore>,
}

struct PersistMem(Share<PersistMemInner>, Uniq);

impl PersistMem {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(
            Share::new(PersistMemInner {
                tls: None,
                priv_keys: HashMap::new(),
                agent_info: HashMap::new(),
                entries: HashMap::new(),
                ui_cache: UiStore::new(),
            }),
            Uniq::default(),
        ))
    }
}

impl AsKdPersist for PersistMem {
    fn uniq(&self) -> Uniq {
        self.1
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        self.0.close();
        async move {}.boxed()
    }

    fn singleton_tls_config(&self) -> BoxFuture<'static, KdResult<TlsConfig>> {
        let inner = self.0.clone();
        async move {
            match inner
                .share_mut(|i, _| Ok(i.tls.clone()))
                .map_err(KdError::other)?
            {
                None => {
                    let tls = TlsConfig::new_ephemeral().await.map_err(KdError::other)?;
                    inner
                        .share_mut(move |i, _| {
                            if i.tls.is_some() {
                                Ok(i.tls.as_ref().unwrap().clone())
                            } else {
                                i.tls = Some(tls.clone());
                                Ok(tls)
                            }
                        })
                        .map_err(KdError::other)
                }
                Some(tls) => Ok(tls),
            }
        }
        .boxed()
    }

    fn generate_signing_keypair(&self) -> BoxFuture<'static, KdResult<KdHash>> {
        let inner = self.0.clone();
        async move {
            let pk = sodoken::BufWriteSized::new_no_lock();
            let sk = sodoken::BufWriteSized::new_mem_locked().map_err(KdError::other)?;

            sodoken::sign::keypair(pk.clone(), sk.clone())
                .await
                .map_err(KdError::other)?;

            let mut pk_hash = [0; 32];
            pk_hash.copy_from_slice(&pk.read_lock()[0..32]);
            let pk_hash = KdHash::from_coerced_pubkey(pk_hash)
                .await
                .map_err(KdError::other)?;

            let pk_hash_clone = pk_hash.clone();
            inner
                .share_mut(move |i, _| {
                    i.priv_keys.insert(pk_hash_clone, sk.to_read_sized());
                    Ok(())
                })
                .map_err(KdError::other)?;

            Ok(pk_hash)
        }
        .boxed()
    }

    fn sign(&self, pub_key: KdHash, data: &[u8]) -> BoxFuture<'static, KdResult<Arc<[u8; 64]>>> {
        let data = sodoken::BufRead::new_no_lock(data);
        let sk = self
            .0
            .share_mut(|i, _| Ok(i.priv_keys.get(&pub_key).cloned()));

        async move {
            let sk = match sk.map_err(KdError::other)? {
                None => return Err(format!("invalid agent: {:?}", pub_key).into()),
                Some(sk) => sk,
            };
            let sig = <sodoken::BufWriteSized<64>>::new_no_lock();
            sodoken::sign::detached(sig.clone(), data, sk)
                .await
                .map_err(KdError::other)?;
            let mut out = [0; 64];
            out.copy_from_slice(&*sig.read_lock());
            Ok(Arc::new(out))
        }
        .boxed()
    }

    fn store_agent_info(&self, agent_info: KdAgentInfo) -> BoxFuture<'static, KdResult<()>> {
        let root = agent_info.root().clone();
        let root_map = self.0.share_mut(move |i, _| {
            Ok(i.agent_info
                .entry(root)
                .or_insert_with(AgentStore::new)
                .clone())
        });
        async move { root_map.map_err(KdError::other)?.insert(agent_info) }.boxed()
    }

    fn get_agent_info(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> BoxFuture<'static, KdResult<KdAgentInfo>> {
        let store = self.0.share_mut(move |i, _| match i.agent_info.get(&root) {
            Some(store) => Ok(store.clone()),
            None => Err("root not found".into()),
        });
        async move { store.map_err(KdError::other)?.get(&agent) }.boxed()
    }

    fn query_agent_info(&self, root: KdHash) -> BoxFuture<'static, KdResult<Vec<KdAgentInfo>>> {
        let store = self.0.share_mut(move |i, _| match i.agent_info.get(&root) {
            Some(store) => Ok(store.clone()),
            None => Err("root not found".into()),
        });
        async move {
            let store = match store {
                Err(_) => return Ok(vec![]),
                Ok(store) => store,
            };
            store.get_all()
        }
        .boxed()
    }

    fn query_agent_info_near_basis(
        &self,
        root: KdHash,
        basis_loc: u32,
        limit: u32,
    ) -> BoxFuture<'static, KdResult<Vec<KdAgentInfo>>> {
        let store = self.0.share_mut(move |i, _| match i.agent_info.get(&root) {
            Some(store) => Ok(store.clone()),
            None => Err("root not found".into()),
        });
        async move {
            let store = match store {
                Err(_) => return Ok(vec![]),
                Ok(store) => store,
            };
            let mut with_dist = store
                .get_all()?
                .into_iter()
                .map(|info| (info.basis_distance_to_storage(basis_loc.into()), info))
                .collect::<Vec<_>>();
            with_dist.sort_by(|a, b| a.0.cmp(&b.0));
            Ok(with_dist
                .into_iter()
                .map(|(_, info)| info)
                .take(limit as usize)
                .collect())
        }
        .boxed()
    }

    fn query_peer_density(
        &self,
        root: KdHash,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> BoxFuture<'static, KdResult<kitsune_p2p_types::dht::PeerView>> {
        let topo = Topology::standard_epoch_full();
        let store = self.0.share_mut(move |i, _| match i.agent_info.get(&root) {
            Some(store) => Ok(store.clone()),
            None => Err("root not found".into()),
        });
        async move {
            let store = match store {
                Err(_) => return Ok(PeerStrat::default().view(topo.clone(), dht_arc, &[])),
                Ok(store) => store,
            };
            let arcs: Vec<_> = store
                .get_all()?
                .into_iter()
                .map(|v| {
                    let loc = DhtLocation::from(v.agent().as_loc());
                    DhtArc::from_parts(*v.storage_arc(), loc)
                })
                .collect();

            // contains is already checked in the iterator
            Ok(PeerStrat::default().view(topo, dht_arc, arcs.as_slice()))
        }
        .boxed()
    }

    fn store_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        entry: KdEntrySigned,
    ) -> BoxFuture<'static, KdResult<()>> {
        let root2 = root.clone();
        let r = self.0.share_mut(move |i, _| {
            let ui_cache = i.ui_cache.clone();
            let agent_map = i
                .entries
                .entry(root2)
                .or_insert_with(AgentEntryStore::new)
                .clone();
            Ok((ui_cache, agent_map))
        });
        async move {
            let (ui_cache, agent_map) = r.map_err(KdError::other)?;
            let _ = ui_cache.check_add(&root, &entry);
            agent_map.get_mut(agent)?.insert(entry)
        }
        .boxed()
    }

    fn get_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        hash: KdHash,
    ) -> BoxFuture<'static, KdResult<KdEntrySigned>> {
        let agent_map = self.0.share_mut(move |i, _| match i.entries.get(&root) {
            Some(agent_map) => Ok(agent_map.clone()),
            None => Err("root not found".into()),
        });
        async move { agent_map.map_err(KdError::other)?.get(&agent)?.get(&hash) }.boxed()
    }

    fn query_entries(
        &self,
        root: KdHash,
        agent: KdHash,
        _window: TimeWindow,
        _dht_arc: DhtArcSet,
    ) -> BoxFuture<'static, KdResult<Vec<KdEntrySigned>>> {
        // TODO - actually filter

        let agent_map = self.0.share_mut(move |i, _| match i.entries.get(&root) {
            Some(agent_map) => Ok(agent_map.clone()),
            None => Err("root not found".into()),
        });
        async move {
            let agent_map = match agent_map {
                Err(_) => return Ok(vec![]),
                Ok(agent_map) => agent_map,
            };
            agent_map.get(&agent)?.get_all()
        }
        .boxed()
    }

    fn get_ui_file(&self, path: &str) -> BoxFuture<'static, KdResult<(String, Vec<u8>)>> {
        if path == "/favicon.svg" {
            return async move {
                Ok((
                    "image/svg+xml".to_string(),
                    br#"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="256" height="256">
    <path d="M 24 16 L 24 240 L 48 240 L 48 152 L 104 240 L 192 240 L 240 128 L 192 16 L 104 16 L 48 104 L 48 16 L 24 16 z M 128 32 L 128 224 L 64 128 L 128 32 z M 152 32 L 176 32 L 216 128 L 176 224 L 152 224 L 152 32 z " />
</svg>"#.to_vec(),
                ))
            }.boxed();
        } else if path.is_empty() || path == "/" || path == "/index.html" {
            let roots = self
                .0
                .share_mut(|i, _| Ok(i.entries.keys().cloned().collect::<Vec<_>>()));
            return async move {
                let roots = roots
                    .map_err(KdError::other)?
                    .into_iter()
                    .map(|h| format!(r#"<li><a href="/{}/index.html">{}</a></li>"#, h, h))
                    .collect::<Vec<_>>();
                let content = format!(
                    r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="favicon.svg" />
  </head>
  <body>
    <h1>App Index:</h1>
    <ul>
      {}
    </ul>
  </body>
</html>"#,
                    roots.join("\n")
                )
                .into_bytes();
                Ok(("text/html".to_string(), content))
            }
            .boxed();
        }

        let ui_cache = self.0.share_mut(|i, _| Ok(i.ui_cache.clone()));
        let path = path.to_string();
        async move {
            let ui_cache = ui_cache.map_err(KdError::other)?;
            let ui_entry = ui_cache.get(&path)?;
            Ok((ui_entry.mime.clone(), ui_entry.data.to_vec()))
        }
        .boxed()
    }
}
