use holo_hash::{AgentPubKey, DnaHash};
use holochain_sqlite::db::{
    DbKindAuthored, DbKindCache, DbKindConductor, DbKindDht, DbKindP2pAgents, DbKindP2pMetrics,
    DbKindT, DbWrite,
};
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::DbKindWasm;
use holochain_zome_types::cell::CellId;
use kitsune_p2p_bin_data::{KitsuneBinType, KitsuneSpace};
use std::sync::Arc;
use walkdir::WalkDir;

#[tokio::test(flavor = "multi_thread")]
async fn check_schema_migrations_execute() {
    let authored = DbWrite::test_in_mem(DbKindAuthored(Arc::new(CellId::new(
        DnaHash::from_raw_36(vec![1; 36]),
        AgentPubKey::from_raw_36(vec![0; 36]),
    ))))
    .unwrap();
    check_migrations_run(authored, "./src/sql/cell/schema").await;

    // These two actually use the same schema as authored, so if one works, the others should.
    // Run anyway to be safe because the migrations are listed separately.
    let dht = DbWrite::test_in_mem(DbKindDht(Arc::new(DnaHash::from_raw_36(vec![1; 36])))).unwrap();
    check_migrations_run(dht, "./src/sql/cell/schema").await;
    let cache =
        DbWrite::test_in_mem(DbKindCache(Arc::new(DnaHash::from_raw_36(vec![1; 36])))).unwrap();
    check_migrations_run(cache, "./src/sql/cell/schema").await;

    let conductor = DbWrite::test_in_mem(DbKindConductor).unwrap();
    check_migrations_run(conductor, "./src/sql/conductor/schema").await;

    let wasm = DbWrite::test_in_mem(DbKindWasm).unwrap();
    check_migrations_run(wasm, "./src/sql/wasm/schema").await;

    let p2p_metrics =
        DbWrite::test_in_mem(DbKindP2pMetrics(Arc::new(KitsuneSpace::new(vec![1; 36])))).unwrap();
    check_migrations_run(p2p_metrics, "./src/sql/p2p_metrics/schema").await;

    let p2p_agents =
        DbWrite::test_in_mem(DbKindP2pAgents(Arc::new(KitsuneSpace::new(vec![1; 36])))).unwrap();
    check_migrations_run(p2p_agents, "./src/sql/p2p_agent_store/schema").await;
}

async fn check_migrations_run<T: DbKindT>(db: DbWrite<T>, path: &str) {
    let user_version = db
        .read_async(|txn| -> DatabaseResult<u16> {
            let user_version: u16 =
                txn.pragma_query_value(None, "user_version", |row| row.get(0))?;

            Ok(user_version)
        })
        .await
        .unwrap();

    let latest_migration = WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name().to_str().unwrap().contains("-up"))
        .map(|e| {
            e.path()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .split("-")
                .next()
                .unwrap()
                .to_string()
        })
        .map(|prefix| prefix.parse::<u16>().unwrap())
        .max()
        .unwrap_or(0);

    assert_eq!(
        user_version,
        latest_migration + 1,
        "Migrations check failed for: {:?}",
        db.kind()
    );
}
