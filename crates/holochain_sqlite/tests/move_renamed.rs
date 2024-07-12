use holo_hash::{AgentPubKey, DnaHash};
use holochain_p2p::DnaHashExt;
use holochain_sqlite::db::{
    DbKindAuthored, DbKindCache, DbKindConductor, DbKindDht, DbKindP2pAgents, DbKindWasm, DbWrite,
};
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::{DbKindP2pMetrics, DbKindT};
use holochain_zome_types::cell::CellId;
use rusqlite::Connection;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn move_renamed() {
    let dna_hash = DnaHash::from_raw_36(vec![1; 36]);
    let authored = DbKindAuthored(Arc::new(CellId::new(
        dna_hash.clone(),
        AgentPubKey::from_raw_36(vec![7; 36]),
    )));
    let dht = DbKindDht(Arc::new(dna_hash.clone()));
    let cache = DbKindCache(Arc::new(dna_hash.clone()));

    let space = dna_hash.to_kitsune();
    let p2p_agent_store = DbKindP2pAgents(space.clone());
    let p2p_metrics = DbKindP2pMetrics(space.clone());

    let conductor = DbKindConductor;
    let wasm = DbKindWasm;

    let path_prefix = tempfile::TempDir::new().unwrap();
    create_dir_all(path_prefix.path().join("authored")).unwrap();
    create_dir_all(path_prefix.path().join("dht")).unwrap();
    create_dir_all(path_prefix.path().join("cache")).unwrap();
    create_dir_all(path_prefix.path().join("p2p")).unwrap();
    create_dir_all(path_prefix.path().join("conductor")).unwrap();
    create_dir_all(path_prefix.path().join("wasm")).unwrap();

    check_db_kind(
        path_prefix.path(),
        authored.legacy_filename(),
        authored.filename(),
        authored,
    )
    .await;
    check_db_kind(
        path_prefix.path(),
        dht.legacy_filename(),
        dht.filename(),
        dht,
    )
    .await;
    check_db_kind(
        path_prefix.path(),
        cache.legacy_filename(),
        cache.filename(),
        cache,
    )
    .await;
    check_db_kind(
        path_prefix.path(),
        p2p_agent_store.legacy_filename(),
        p2p_agent_store.filename(),
        p2p_agent_store,
    )
    .await;
    check_db_kind(
        path_prefix.path(),
        p2p_metrics.legacy_filename(),
        p2p_metrics.filename(),
        p2p_metrics,
    )
    .await;
    check_db_kind(
        path_prefix.path(),
        conductor.legacy_filename(),
        conductor.filename(),
        conductor,
    )
    .await;
    check_db_kind(
        path_prefix.path(),
        wasm.legacy_filename(),
        wasm.filename(),
        wasm,
    )
    .await;
}

async fn check_db_kind<Kind: DbKindT + Send + Sync + 'static>(
    path_prefix: &Path,
    legacy_filename: Option<PathBuf>,
    filename: PathBuf,
    kind: Kind,
) {
    {
        let path = path_prefix.join(legacy_filename.clone().unwrap_or_else(|| filename.clone()));
        let conn = Connection::open(path).unwrap();

        conn.execute("CREATE TABLE hello (value TEXT)", ()).unwrap();
        conn.execute("INSERT INTO hello (value) VALUES ('test')", ())
            .unwrap();

        conn.close().unwrap();
    }

    let f = DbWrite::open(path_prefix, kind).unwrap();

    // Should be in the new location, whether it was moved or not
    assert_eq!(f.path().to_owned(), path_prefix.join(filename));

    // Should not be in the old location, if an old location was defined
    if let Some(legacy_filename) = legacy_filename {
        assert!(!path_prefix.join(legacy_filename).exists());
    }

    // Should be able to read existing data
    let msg = f
        .read_async(|txn| -> DatabaseResult<String> {
            let msg =
                txn.query_row("SELECT value FROM hello", [], |row| row.get::<_, String>(0))?;
            Ok(msg)
        })
        .await
        .unwrap();
    assert_eq!(msg, "test".to_string());
}
