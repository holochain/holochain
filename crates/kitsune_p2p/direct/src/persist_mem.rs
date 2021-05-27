//! in-memory persistence module for kitsune direct

use crate::*;
use futures::future::{BoxFuture, FutureExt};
use kitsune_p2p::event::MetricDatum;
use kitsune_p2p::event::MetricQuery;
use kitsune_p2p::event::MetricQueryAnswer;
use kitsune_p2p::KitsuneAgent;
use kitsune_p2p_types::dht_arc::DhtArc;
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use types::kdagent::*;
use types::kdentry::KdEntry;
use types::kdhash::KdHash;
use types::persist::*;

/// construct a new in-memory persistence module for kitsune direct
pub fn new_persist_mem() -> KdPersist {
    KdPersist(PersistMem::new())
}

// -- private -- //

struct PersistMemInner {
    tls: Option<TlsConfig>,
    priv_keys: HashMap<KdHash, sodoken::Buffer>,
    agent_info: HashMap<KdHash, HashMap<KdHash, KdAgentInfo>>,
    entries: HashMap<KdHash, HashMap<KdHash, HashMap<KdHash, KdEntry>>>,
}

struct PersistMem(Share<PersistMemInner>, Uniq);

impl PersistMem {
    pub fn new() -> Arc<Self> {
        Arc::new(PersistMem(
            Share::new(PersistMemInner {
                tls: None,
                priv_keys: HashMap::new(),
                agent_info: HashMap::new(),
                entries: HashMap::new(),
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

    fn singleton_tls_config(&self) -> BoxFuture<'static, KitsuneResult<TlsConfig>> {
        let inner = self.0.clone();
        async move {
            match inner.share_mut(|i, _| Ok(i.tls.clone()))? {
                None => {
                    let tls = TlsConfig::new_ephemeral().await?;
                    inner.share_mut(move |i, _| {
                        if i.tls.is_some() {
                            Ok(i.tls.as_ref().unwrap().clone())
                        } else {
                            i.tls = Some(tls.clone());
                            Ok(tls)
                        }
                    })
                }
                Some(tls) => Ok(tls),
            }
        }
        .boxed()
    }

    fn generate_signing_keypair(&self) -> BoxFuture<'static, KitsuneResult<KdHash>> {
        let inner = self.0.clone();
        async move {
            let mut pk = Buffer::new(sodoken::sign::SIGN_PUBLICKEYBYTES);
            let mut sk = Buffer::new_memlocked(sodoken::sign::SIGN_SECRETKEYBYTES)
                .map_err(KitsuneError::other)?;

            sodoken::sign::sign_keypair(&mut pk, &mut sk)
                .await
                .map_err(KitsuneError::other)?;

            let pk = pk.read_lock().to_vec();
            let pk_hash = KdHash::from_coerced_pubkey(&pk).await?;

            let pk_hash_clone = pk_hash.clone();
            inner.share_mut(move |i, _| {
                i.priv_keys.insert(pk_hash_clone, sk);
                Ok(())
            })?;

            Ok(pk_hash)
        }
        .boxed()
    }

    fn sign(
        &self,
        pub_key: KdHash,
        data: &[u8],
    ) -> BoxFuture<'static, KitsuneResult<Arc<[u8; 64]>>> {
        let data = Buffer::from_ref(data);
        let sk = self
            .0
            .share_mut(|i, _| Ok(i.priv_keys.get(&pub_key).cloned()));

        async move {
            let sk = match sk? {
                None => return Err(format!("invalid agent: {:?}", pub_key).into()),
                Some(sk) => sk,
            };
            let mut sig = Buffer::new(64);
            sodoken::sign::sign_detached(&mut sig, &data, &sk)
                .await
                .map_err(KitsuneError::other)?;
            let mut out = [0; 64];
            out.copy_from_slice(&*sig.read_lock());
            Ok(Arc::new(out))
        }
        .boxed()
    }

    fn store_agent_info(&self, agent_info: KdAgentInfo) -> BoxFuture<'static, KitsuneResult<()>> {
        let r = self.0.share_mut(move |i, _| {
            let root = agent_info.root.clone();
            let agent = agent_info.agent.clone();

            let root_map = i.agent_info.entry(root).or_insert_with(HashMap::new);

            match root_map.entry(agent) {
                Entry::Occupied(mut e) => {
                    if e.get().signed_at_ms < agent_info.signed_at_ms {
                        e.insert(agent_info);
                    }
                }
                Entry::Vacant(e) => {
                    e.insert(agent_info);
                }
            }

            Ok(())
        });
        async move { r }.boxed()
    }

    fn get_agent_info(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> BoxFuture<'static, KitsuneResult<KdAgentInfo>> {
        let r = self.0.share_mut(|i, _| {
            let root_map = match i.agent_info.get(&root) {
                None => return Err("root not found".into()),
                Some(r) => r,
            };

            match root_map.get(&agent) {
                None => Err("agent not found".into()),
                Some(a) => Ok(a.clone()),
            }
        });
        async move { r }.boxed()
    }

    fn query_agent_info(
        &self,
        root: KdHash,
    ) -> BoxFuture<'static, KitsuneResult<Vec<KdAgentInfo>>> {
        let r = self.0.share_mut(|i, _| {
            let root_map = match i.agent_info.get(&root) {
                None => return Err("root not found".into()),
                Some(r) => r,
            };

            Ok(root_map.values().cloned().collect())
        });
        async move { r }.boxed()
    }

    fn put_metric_datum(
        &self,
        _agent: KitsuneAgent,
        _datum: MetricDatum,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        todo!()
    }

    fn query_metrics(
        &self,
        _query: MetricQuery,
    ) -> BoxFuture<'static, KitsuneResult<MetricQueryAnswer>> {
        todo!()
    }

    fn store_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        entry: KdEntry,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let r = self.0.share_mut(move |i, _| {
            let hash = entry.hash().clone();

            let root_map = i.entries.entry(root).or_insert_with(HashMap::new);
            let agent_map = root_map.entry(agent).or_insert_with(HashMap::new);

            match agent_map.entry(hash) {
                Entry::Occupied(mut e) => {
                    e.insert(entry);
                }
                Entry::Vacant(e) => {
                    e.insert(entry);
                }
            }

            Ok(())
        });
        async move { r }.boxed()
    }

    fn get_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        hash: KdHash,
    ) -> BoxFuture<'static, KitsuneResult<KdEntry>> {
        let r = self.0.share_mut(|i, _| {
            let root_map = match i.entries.get(&root) {
                None => return Err("root not found".into()),
                Some(r) => r,
            };

            let agent_map = match root_map.get(&agent) {
                None => return Err("agent not found".into()),
                Some(r) => r,
            };

            match agent_map.get(&hash) {
                None => Err("entry not found".into()),
                Some(e) => Ok(e.clone()),
            }
        });
        async move { r }.boxed()
    }

    fn query_entries(
        &self,
        root: KdHash,
        agent: KdHash,
        _created_at_start_s: f32,
        _created_at_end_s: f32,
        _dht_arc: DhtArc,
    ) -> BoxFuture<'static, KitsuneResult<Vec<KdEntry>>> {
        // TODO - actually filter

        let r = self.0.share_mut(|i, _| {
            let root_map = match i.entries.get(&root) {
                None => return Err("root not found".into()),
                Some(r) => r,
            };

            let agent_map = match root_map.get(&agent) {
                None => return Err("agent not found".into()),
                Some(r) => r,
            };

            Ok(agent_map.values().cloned().collect())
        });
        async move { r }.boxed()
    }
}
