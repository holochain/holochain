use super::*;
use rusqlite::*;

pub(crate) trait AsPersist: 'static + Send + Sync {
    fn store_sign_pair(
        &self,
        pk: KdHash,
        sk: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    fn get_sign_secret(&self, pk: KdHash) -> ghost_actor::GhostFuture<sodoken::Buffer, KdError>;

    fn store_agent_info(
        &self,
        root_agent: KdHash,
        agent_info_signed: agent_store::AgentInfoSigned,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    fn get_agent_info(
        &self,
        root_agent: KdHash,
        agent: KdHash,
    ) -> ghost_actor::GhostFuture<agent_store::AgentInfoSigned, KdError>;

    fn query_agent_info(
        &self,
        root_agent: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<agent_store::AgentInfoSigned>, KdError>;

    fn store_entry(
        &self,
        root_agent: KdHash,
        entry: KdEntry,
    ) -> ghost_actor::GhostFuture<(), KdError>;

    fn get_entry(
        &self,
        root_agent: KdHash,
        hash: KdHash,
    ) -> ghost_actor::GhostFuture<KdEntry, KdError>;

    fn query_entries(
        &self,
        root_agent: KdHash,
        created_at_start: DateTime<Utc>,
        created_at_end: DateTime<Utc>,
        dht_arc: dht_arc::DhtArc,
    ) -> ghost_actor::GhostFuture<Vec<KdEntry>, KdError>;

    fn list_left_links(
        &self,
        root_agent: KdHash,
        target: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<KdHash>, KdError>;

    ghost_actor::ghost_box_trait_fns!(AsPersist);
}
ghost_actor::ghost_box_trait!(AsPersist);

pub(crate) struct Persist(Box<dyn AsPersist>);
ghost_actor::ghost_box_new_type!(Persist);

#[allow(dead_code)]
impl Persist {
    pub fn store_sign_pair(
        &self,
        pk: KdHash,
        sk: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsPersist::store_sign_pair(&*self.0, pk, sk)
    }

    pub fn get_sign_secret(
        &self,
        pk: KdHash,
    ) -> ghost_actor::GhostFuture<sodoken::Buffer, KdError> {
        AsPersist::get_sign_secret(&*self.0, pk)
    }

    pub fn store_agent_info(
        &self,
        root_agent: KdHash,
        agent_info_signed: agent_store::AgentInfoSigned,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsPersist::store_agent_info(&*self.0, root_agent, agent_info_signed)
    }

    pub fn get_agent_info(
        &self,
        root_agent: KdHash,
        agent: KdHash,
    ) -> ghost_actor::GhostFuture<agent_store::AgentInfoSigned, KdError> {
        AsPersist::get_agent_info(&*self.0, root_agent, agent)
    }

    pub fn query_agent_info(
        &self,
        root_agent: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<agent_store::AgentInfoSigned>, KdError> {
        AsPersist::query_agent_info(&*self.0, root_agent)
    }

    pub fn store_entry(
        &self,
        root_agent: KdHash,
        entry: KdEntry,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        AsPersist::store_entry(&*self.0, root_agent, entry)
    }

    pub fn get_entry(
        &self,
        root_agent: KdHash,
        hash: KdHash,
    ) -> ghost_actor::GhostFuture<KdEntry, KdError> {
        AsPersist::get_entry(&*self.0, root_agent, hash)
    }

    pub fn query_entries(
        &self,
        root_agent: KdHash,
        created_at_start: DateTime<Utc>,
        created_at_end: DateTime<Utc>,
        dht_arc: dht_arc::DhtArc,
    ) -> ghost_actor::GhostFuture<Vec<KdEntry>, KdError> {
        AsPersist::query_entries(
            &*self.0,
            root_agent,
            created_at_start,
            created_at_end,
            dht_arc,
        )
    }

    pub fn list_left_links(
        &self,
        root_agent: KdHash,
        target: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<KdHash>, KdError> {
        AsPersist::list_left_links(&*self.0, root_agent, target)
    }
}

pub(crate) async fn spawn_persist_sqlcipher(config: KdConfig) -> KdResult<Persist> {
    SqlPersist::new(config).await
}

struct SqlPersistInner {
    con: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SqlPersist(ghost_actor::GhostActor<SqlPersistInner>);

const KEY_PRAGMA_LEN: usize = 83;
const KEY_PRAGMA: &[u8; KEY_PRAGMA_LEN] =
    br#"PRAGMA key = "x'0000000000000000000000000000000000000000000000000000000000000000'";"#;

/// write a sqlcipher key pragma maintaining mem protection
async fn secure_write_key_pragma(passphrase: &sodoken::Buffer) -> KdResult<sodoken::Buffer> {
    // first, hash the passphrase
    let mut key_buf = sodoken::Buffer::new_memlocked(32)?;
    sodoken::hash::generichash(&mut key_buf, &passphrase, None).await?;

    // now write the pragma line
    let key_pragma = sodoken::Buffer::new_memlocked(KEY_PRAGMA_LEN)?;

    {
        use std::io::Write;
        let mut key_pragma = key_pragma.write_lock();
        key_pragma.copy_from_slice(KEY_PRAGMA);
        let mut c = std::io::Cursor::new(&mut key_pragma[16..80]);
        for b in &*key_buf.read_lock() {
            write!(c, "{:02X}", b).map_err(KdError::other)?;
        }
    }

    Ok(key_pragma)
}

impl SqlPersist {
    #[allow(clippy::new_ret_no_self)]
    pub async fn new(config: KdConfig) -> KdResult<Persist> {
        let con = Connection::open(match config.persist_path {
            Some(p) => p,
            None => std::path::Path::new(":memory:").to_path_buf(),
        })?;

        // set encryption key
        let key_pragma = secure_write_key_pragma(&config.unlock_passphrase).await?;
        con.execute(
            std::str::from_utf8(&*key_pragma.read_lock()).unwrap(),
            NO_PARAMS,
        )?;

        // set to faster write-ahead-log mode
        con.pragma_update(None, "journal_mode", &"WAL".to_string())?;

        // create the private key table
        con.execute(
            "CREATE TABLE IF NOT EXISTS sign_keypairs (
                pub_key       TEXT UNIQUE PRIMARY KEY NOT NULL,
                sec_key       BLOB NOT NULL
            ) WITHOUT ROWID;",
            NO_PARAMS,
        )?;

        // create the agent_info table
        // NOTE - NOT using `WITHOUT ROWID` because row size may be > 200 bytes
        // TODO - should we also store decoded agent_info fields?
        //        that would give us additional query functionality.
        con.execute(
            "CREATE TABLE IF NOT EXISTS agent_info (
                root_agent              TEXT NOT NULL,
                agent                   TEXT NOT NULL,
                signature               BLOB NOT NULL,
                agent_info              BLOB NOT NULL,
                signed_at_epoch_ms      INTEGER NOT NULL,
                expires_at_epoch_ms     INTEGER NOT NULL,
                CONSTRAINT agent_info_pk PRIMARY KEY (root_agent, agent)
            );",
            NO_PARAMS,
        )?;

        // create the entries table
        // NOTE - NOT using `WITHOUT ROWID` because row size may be > 200 bytes
        con.execute(
            "CREATE TABLE IF NOT EXISTS entries (
                root_agent    TEXT NOT NULL,
                hash          TEXT NOT NULL,
                created_at    TEXT NOT NULL,
                dht_loc       INT NOT NULL,
                left_link     TEXT NOT NULL,
                bytes         BLOB NOT NULL,
                CONSTRAINT entries_pk PRIMARY KEY (root_agent, hash)
            );",
            NO_PARAMS,
        )?;

        // created_at + dht_loc index for entries
        con.execute(
            "CREATE INDEX IF NOT EXISTS entries_query_idx ON entries (
                root_agent, created_at, dht_loc
            );",
            NO_PARAMS,
        )?;

        // left_link index for entries
        con.execute(
            "CREATE INDEX IF NOT EXISTS entries_left_link_idx ON entries (
                root_agent, left_link
            );",
            NO_PARAMS,
        )?;

        let (actor, driver) = ghost_actor::GhostActor::new(SqlPersistInner { con });
        tokio::task::spawn(driver);
        Ok(Persist(Box::new(Self(actor))))
    }
}

impl AsPersist for SqlPersist {
    ghost_actor::ghost_box_trait_impl_fns!(AsPersist);

    fn store_sign_pair(
        &self,
        pk: KdHash,
        sk: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        self.0.invoke(move |inner| {
            let tx = inner.con.transaction()?;

            {
                let mut ins =
                    tx.prepare("INSERT INTO sign_keypairs (pub_key, sec_key) VALUES (?1, ?2);")?;

                // TODO - the same dance we did with the encryption key above
                ins.execute(params![pk.as_ref(), &*sk.read_lock()])?;
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_sign_secret(&self, pk: KdHash) -> ghost_actor::GhostFuture<sodoken::Buffer, KdError> {
        self.0.invoke(move |inner| {
            let buffer = sodoken::Buffer::new_memlocked(64)?;
            inner.con.query_row(
                "SELECT sec_key FROM sign_keypairs WHERE pub_key = ?1 LIMIT 1;",
                params![pk.as_ref()],
                |row| {
                    // TODO - how do we make sure this stays secure??
                    if let types::ValueRef::Blob(b) = row.get_raw(0) {
                        buffer.write_lock().copy_from_slice(b);
                        Ok(())
                    } else {
                        Err(Error::ToSqlConversionFailure(Box::new(KdError::from(
                            "bad type",
                        ))))
                    }
                },
            )?;
            Ok(buffer)
        })
    }

    fn store_agent_info(
        &self,
        root_agent: KdHash,
        agent_info_signed: agent_store::AgentInfoSigned,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        self.0.invoke(move |inner| {
            let tx = inner.con.transaction()?;

            let agent: KdHash = agent_info_signed.as_agent_ref().into();
            let sig: &[u8] = &agent_info_signed.as_signature_ref().0;
            let info_bytes: &[u8] = agent_info_signed.as_agent_info_ref();

            use std::convert::TryInto;
            let info: agent_store::AgentInfo = (&agent_info_signed).try_into()?;

            if agent_info_signed.as_agent_ref() != info.as_agent_ref() {
                return Err("inner/outer agent mismatch".into());
            }

            let signed_at_epoch_ms = info.signed_at_ms();
            let expires_at_epoch_ms = signed_at_epoch_ms + info.expires_after_ms();

            let exists = {
                let mut exists = tx.prepare(
                    "SELECT TRUE AS 'exists' FROM agent_info
                    WHERE root_agent = ?1
                    AND agent = ?2;",
                )?;
                exists.exists(params![root_agent.as_ref(), agent.as_ref()])?
            };

            if exists {
                let mut upd = tx.prepare(
                    "UPDATE agent_info SET
                            signature = ?1,
                            agent_info = ?2,
                            signed_at_epoch_ms = ?3,
                            expires_at_epoch_ms = ?4
                        WHERE root_agent = ?5
                        AND agent = ?6",
                )?;

                upd.execute(params![
                    sig,
                    info_bytes,
                    signed_at_epoch_ms as i64,
                    expires_at_epoch_ms as i64,
                    root_agent.as_ref(),
                    agent.as_ref()
                ])?;
            } else {
                let mut ins = tx.prepare(
                    "INSERT INTO agent_info (
                            root_agent,
                            agent,
                            signature,
                            agent_info,
                            signed_at_epoch_ms,
                            expires_at_epoch_ms
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                )?;

                ins.execute(params![
                    root_agent.as_ref(),
                    agent.as_ref(),
                    sig,
                    info_bytes,
                    signed_at_epoch_ms as i64,
                    expires_at_epoch_ms as i64
                ])?;
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_agent_info(
        &self,
        root_agent: KdHash,
        agent: KdHash,
    ) -> ghost_actor::GhostFuture<agent_store::AgentInfoSigned, KdError> {
        self.0.invoke(move |inner| {
            let (sig, info) = inner.con.query_row(
                "SELECT signature, agent_info FROM agent_info WHERE root_agent = ?1 AND agent = ?2 LIMIT 1;",
                params![root_agent.as_ref(), agent.as_ref()],
                |row| {
                    let sig: Vec<u8> = row.get(0)?;
                    let info: Vec<u8> = row.get(1)?;
                    Ok((sig, info))
                },
            )?;
            let out = agent_store::AgentInfoSigned::try_new(agent.into(), sig.into(), info)?;
            Ok(out)
        })
    }

    fn query_agent_info(
        &self,
        root_agent: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<agent_store::AgentInfoSigned>, KdError> {
        self.0.invoke(move |inner| {
            let tx = inner.con.transaction()?;

            let out = {
                let mut stmt = tx.prepare(
                    "SELECT agent, signature, agent_info FROM agent_info WHERE root_agent = ?1;",
                )?;

                let res = stmt.query_map(params![root_agent.as_ref()], |row| {
                    let agent: String = row.get(0)?;
                    let sig: Vec<u8> = row.get(1)?;
                    let info: Vec<u8> = row.get(2)?;
                    Ok((agent, sig, info))
                })?;

                let mut out = Vec::new();

                for r in res {
                    let (agent, sig, info) = r?;
                    // TODO - move to async block && check
                    let agent = KdHash::from_str_unchecked(&agent);
                    out.push(agent_store::AgentInfoSigned::try_new(
                        agent.into(),
                        sig.into(),
                        info,
                    )?);
                }

                out
            };

            tx.rollback()?;

            Ok(out)
        })
    }

    fn store_entry(
        &self,
        root_agent: KdHash,
        entry: KdEntry,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        self.0.invoke(move |inner| {
            let tx = inner.con.transaction()?;

            {
                let mut ins =
                    tx.prepare("INSERT OR IGNORE INTO entries (root_agent, hash, created_at, dht_loc, left_link, bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6);")?;

                ins.execute(params![
                    root_agent.as_ref(),
                    entry.hash().as_ref(),
                    entry.create(),
                    entry.hash().get_loc(),
                    entry.left_link().as_ref(),
                    entry.as_ref(),
                ])?;
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_entry(
        &self,
        root_agent: KdHash,
        hash: KdHash,
    ) -> ghost_actor::GhostFuture<KdEntry, KdError> {
        let fut = self.0.invoke(move |inner| {
            KdResult::Ok(inner.con.query_row(
                "SELECT bytes FROM entries WHERE root_agent = ?1 AND hash = ?2 LIMIT 1;",
                params![root_agent.as_ref(), hash.as_ref()],
                |row| {
                    let bytes: Vec<u8> = row.get(0)?;
                    Ok(bytes)
                },
            )?)
        });

        ghost_actor::resp(async move {
            let bytes = fut.await?;
            KdEntry::from_raw_bytes_validated(bytes.into_boxed_slice()).await
        })
    }

    fn query_entries(
        &self,
        root_agent: KdHash,
        _created_at_start: DateTime<Utc>,
        _created_at_end: DateTime<Utc>,
        arc: dht_arc::DhtArc,
    ) -> ghost_actor::GhostFuture<Vec<KdEntry>, KdError> {
        if arc.half_length == 0 {
            return ghost_actor::resp(async move { Ok(Vec::with_capacity(0)) });
        }

        let fut = self.0.invoke(move |inner| {
            let tx = inner.con.transaction()?;

            let res = {
                let dht_arc::ArcRange { start, end } = arc.range();

                let start = match start {
                    std::ops::Bound::Included(u) => u,
                    _ => unreachable!(),
                };

                let end = match end {
                    std::ops::Bound::Included(u) => u,
                    _ => unreachable!(),
                };

                // TODO - fix these queries when the start to matter

                let mut stmt = if start <= end {
                    tx.prepare(
                        "SELECT bytes FROM entries
                            WHERE root_agent = ?1
                            --AND created_at >= ?2
                            --AND created_at <= ?3
                            --AND dht_loc >= ?4
                            --AND dht_loc <= ?5;",
                    )?
                } else {
                    tx.prepare(
                        "SELECT bytes FROM entries
                            WHERE root_agent = ?1
                            --AND created_at >= ?2
                            --AND created_at <= ?3
                            --AND dht_loc <= ?4
                            --AND dht_loc >= ?5;",
                    )?
                };

                let params = params![
                    root_agent.as_ref(),
                    //created_at_start,
                    //created_at_end,
                    //start,
                    //end,
                ];
                /*
                for p in params.iter() {
                    println!("PARAM: {:?}", p.to_sql());
                }
                */

                let res = stmt.query_map(params, |row| {
                    let bytes: Vec<u8> = row.get(0)?;
                    Ok(bytes.into_boxed_slice())
                })?;

                let mut out = Vec::new();

                for r in res {
                    out.push(r?);
                }

                out
            };

            tx.rollback()?;

            KdResult::Ok(res)
        });

        ghost_actor::resp(async move {
            let res = fut.await?;

            let mut out = Vec::new();

            for r in res {
                out.push(KdEntry::from_raw_bytes_validated(r).await?);
            }

            Ok(out)
        })
    }

    fn list_left_links(
        &self,
        root_agent: KdHash,
        target: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<KdHash>, KdError> {
        self.0.invoke(move |inner| {
            let mut stmt = inner.con.prepare(
                "SELECT hash FROM entries
                    WHERE root_agent = ?1
                    AND left_link = ?2
                    ;",
            )?;
            let res = stmt.query_map(params![root_agent.as_ref(), target.as_ref()], |row| {
                let hash: String = row.get(0)?;
                // TODO - move to async block && check
                Ok(KdHash::from_str_unchecked(&hash))
            })?;

            let mut out = Vec::new();

            for r in res {
                out.push(r?);
            }

            Ok(out)
        })
    }
}
