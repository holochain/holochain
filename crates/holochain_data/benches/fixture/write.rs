//! Writes generated records into a file-backed DHT database.

use super::{generate, FixtureConfig, Generated};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DhtOpHash, DnaHash, EntryHash};
use holochain_data::dht::{
    InsertChainOp, InsertDeletedLink, InsertDeletedRecord, InsertLimboChainOp, InsertLimboWarrant,
    InsertLink, InsertUpdatedRecord, InsertWarrant,
};
use holochain_data::kind::Dht;
use holochain_data::{open_db, DbWrite, HolochainDataConfig};
use holochain_integrity_types::action::{ActionData, RecordValidity};
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_timestamp::Timestamp;
use holochain_types::op::produce_ops_from_record;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;
use std::sync::Arc;

/// Keys sampled from the fixture, so benchmarks hit rows that exist.
pub struct FixtureKeys {
    pub local_author: AgentPubKey,
    pub authors: Vec<AgentPubKey>,
    pub action_hashes: Vec<ActionHash>,
    pub entry_hashes: Vec<EntryHash>,
    pub updated_action_hashes: Vec<ActionHash>,
    pub deleted_action_hashes: Vec<ActionHash>,
    pub link_bases: Vec<AnyLinkableHash>,
    pub create_link_hashes: Vec<ActionHash>,
    pub op_hashes: Vec<DhtOpHash>,
    pub limbo_op_hashes: Vec<DhtOpHash>,
    pub outcome_integrated: Vec<(ActionHash, i64)>,
    pub outcome_limbo_decided: Vec<(ActionHash, i64)>,
    pub outcome_limbo_pending: Vec<(ActionHash, i64)>,
    pub outcome_missing: Vec<(ActionHash, i64)>,
    pub local_op_hashes: Vec<DhtOpHash>,
    pub warrantees: Vec<AgentPubKey>,
    pub min_timestamp: i64,
    pub max_timestamp: i64,
    pub limbo_rows: u64,
}

/// A built fixture; drops the temp dir with the database.
pub struct Fixture {
    pub size: usize,
    pub db: DbWrite<Dht>,
    pub keys: FixtureKeys,
    _dir: tempfile::TempDir,
}

/// Max sample keys kept per category.
const SAMPLE: usize = 256;
/// Share of ops that stay in limbo instead of being integrated.
const LIMBO_SHARE: f64 = 0.10;
const WARRANTS: usize = 20;

fn keep<T: Clone>(v: &mut Vec<T>, item: &T, rng: &mut StdRng, seen: usize) {
    // Reservoir sampling keeps a uniform sample without storing all keys.
    if v.len() < SAMPLE {
        v.push(item.clone());
    } else {
        let j = rng.random_range(0..seen);
        if j < SAMPLE {
            v[j] = item.clone();
        }
    }
}

/// Build the database for `cfg` in a fresh temp dir.
pub async fn build(cfg: FixtureConfig) -> Fixture {
    let g: Generated = generate(cfg);
    super::check_generated(cfg, &g);
    let mut rng = StdRng::seed_from_u64(cfg.seed ^ 0x5eed);

    let dir = tempfile::TempDir::new().expect("temp dir");
    let dna_hash = DnaHash::from_raw_36(vec![7u8; 36]);
    let db = open_db(
        dir.path(),
        Dht::new(Arc::new(dna_hash)),
        HolochainDataConfig::new(),
    )
    .await
    .expect("open dht db");

    let mut keys = FixtureKeys {
        local_author: g.local_author.clone(),
        authors: g.chains.iter().map(|c| c.author.clone()).collect(),
        action_hashes: Vec::new(),
        entry_hashes: Vec::new(),
        updated_action_hashes: Vec::new(),
        deleted_action_hashes: Vec::new(),
        link_bases: g.link_bases.clone(),
        create_link_hashes: Vec::new(),
        op_hashes: Vec::new(),
        limbo_op_hashes: Vec::new(),
        outcome_integrated: Vec::new(),
        outcome_limbo_decided: Vec::new(),
        outcome_limbo_pending: Vec::new(),
        outcome_missing: Vec::new(),
        local_op_hashes: Vec::new(),
        warrantees: Vec::new(),
        min_timestamp: g.min_timestamp,
        max_timestamp: g.max_timestamp,
        limbo_rows: 0,
    };

    let mut tx = db.begin().await.expect("begin");
    let mut seen_actions = 0usize;
    let mut seen_ops = 0usize;
    let mut integrated_at = g.min_timestamp;
    let mut local_create_hashes: Vec<ActionHash> = Vec::new();

    for chain in &g.chains {
        let is_local = chain.author == g.local_author;
        for record in &chain.records {
            let sah = &record.signed_action;
            let action = sah.action();
            let hash = sah.as_hash();
            seen_actions += 1;

            // Entry
            if let Some(entry) = record.entry.as_option() {
                let entry_hash = action.entry_hash().expect("entry implies hash");
                let vis = action.entry_visibility().copied().unwrap_or_default();
                if vis == EntryVisibility::Private {
                    tx.insert_private_entry(entry_hash, &chain.author, entry)
                        .await
                        .expect("private entry");
                } else {
                    tx.insert_entry(entry_hash, entry).await.expect("entry");
                }
            }

            // Action
            tx.insert_action(sah, Some(RecordValidity::Accepted))
                .await
                .expect("action");

            // Index tables and key sampling
            match &action.data {
                ActionData::Create(c) => {
                    keep(&mut keys.action_hashes, hash, &mut rng, seen_actions);
                    if c.entry_type.visibility() == &EntryVisibility::Public {
                        keep(
                            &mut keys.entry_hashes,
                            &c.entry_hash,
                            &mut rng,
                            seen_actions,
                        );
                    }
                    if is_local {
                        local_create_hashes.push(hash.clone());
                    }
                }
                ActionData::Update(u) => {
                    tx.insert_updated_record_index(InsertUpdatedRecord {
                        action_hash: hash,
                        original_action_hash: &u.original_action_address,
                        original_entry_hash: &u.original_entry_address,
                    })
                    .await
                    .expect("updated record");
                    keep(
                        &mut keys.updated_action_hashes,
                        &u.original_action_address,
                        &mut rng,
                        seen_actions,
                    );
                }
                ActionData::Delete(d) => {
                    tx.insert_deleted_record_index(InsertDeletedRecord {
                        action_hash: hash,
                        deletes_action_hash: &d.deletes_address,
                        deletes_entry_hash: &d.deletes_entry_address,
                    })
                    .await
                    .expect("deleted record");
                    keep(
                        &mut keys.deleted_action_hashes,
                        &d.deletes_address,
                        &mut rng,
                        seen_actions,
                    );
                }
                ActionData::CreateLink(l) => {
                    tx.insert_link_index(InsertLink {
                        action_hash: hash,
                        base_hash: &l.base_address,
                        zome_index: l.zome_index.0,
                        link_type: l.link_type.0,
                        tag: Some(l.tag.0.as_slice()),
                    })
                    .await
                    .expect("link");
                    keep(&mut keys.create_link_hashes, hash, &mut rng, seen_actions);
                }
                ActionData::DeleteLink(d) => {
                    tx.insert_deleted_link_index(InsertDeletedLink {
                        action_hash: hash,
                        create_link_hash: &d.link_add_address,
                    })
                    .await
                    .expect("deleted link");
                }
                _ => {}
            }

            // Ops
            let entry_len = record.entry.as_option().map_or(0, |e| match e {
                holochain_integrity_types::entry::Entry::App(b) => b.bytes().len(),
                _ => 64,
            });
            let serialized_size = (entry_len + 300) as u32;
            for op in produce_ops_from_record(record) {
                seen_ops += 1;
                let op_type = i64::from(op.op_type);
                let to_limbo = !is_local && rng.random_bool(LIMBO_SHARE);
                let when_received = Timestamp::from_micros(
                    action.timestamp().as_micros() + rng.random_range(1_000..60_000_000),
                );
                if to_limbo {
                    tx.insert_limbo_chain_op(InsertLimboChainOp {
                        op_hash: &op.op_hash,
                        action_hash: op.action_hash(),
                        op_type,
                        basis_hash: &op.basis_hash,
                        storage_center_loc: op.storage_center_loc,
                        require_receipt: rng.random_bool(0.5),
                        when_received,
                        serialized_size,
                    })
                    .await
                    .expect("limbo op");
                    keys.limbo_rows += 1;
                    keep(&mut keys.limbo_op_hashes, &op.op_hash, &mut rng, seen_ops);
                } else {
                    integrated_at += rng.random_range(1..2_000);
                    tx.insert_chain_op(InsertChainOp {
                        op_hash: &op.op_hash,
                        action_hash: op.action_hash(),
                        op_type,
                        basis_hash: &op.basis_hash,
                        storage_center_loc: op.storage_center_loc,
                        validation_status: RecordValidity::Accepted,
                        locally_validated: true,
                        require_receipt: !is_local && rng.random_bool(0.05),
                        when_received,
                        when_integrated: Timestamp::from_micros(integrated_at),
                        serialized_size,
                    })
                    .await
                    .expect("chain op");
                    keep(&mut keys.op_hashes, &op.op_hash, &mut rng, seen_ops);
                    if is_local {
                        keys.local_op_hashes.push(op.op_hash.clone());
                        let published = rng.random_bool(0.8);
                        tx.insert_chain_op_publish(
                            &op.op_hash,
                            published.then(|| Timestamp::from_micros(integrated_at)),
                            None,
                            None,
                        )
                        .await
                        .expect("publish row");
                        for k in 0..3u8 {
                            let mut receipt_hash = op.op_hash.get_raw_36().to_vec();
                            receipt_hash[0] ^= k + 1;
                            let receipt_hash = DhtOpHash::from_raw_36(receipt_hash);
                            let blob = vec![k; 200];
                            tx.insert_validation_receipt(
                                &receipt_hash,
                                &op.op_hash,
                                &blob,
                                Timestamp::from_micros(integrated_at + 10),
                            )
                            .await
                            .expect("receipt");
                        }
                    }
                }
            }
        }
    }

    // Cap grants on five of the local author's creates (the read path only
    // looks at the CapGrant row and the Action row, not the entry).
    for (i, hash) in local_create_hashes.iter().take(5).enumerate() {
        tx.insert_cap_grant(hash, (i % 3) as i64, Some("bench"))
            .await
            .expect("cap grant");
    }

    // Warrants against the first five authors, half integrated, half limbo.
    let warrant_author = AgentPubKey::from_raw_36(vec![9u8; 36]);
    let mut warrantees: HashSet<AgentPubKey> = HashSet::new();
    let warrant_author_count = keys.authors.len().min(5);
    for i in 0..WARRANTS {
        let warrantee = &keys.authors[i % warrant_author_count];
        warrantees.insert(warrantee.clone());
        let hash = DhtOpHash::from_raw_36({
            let mut v = vec![0u8; 36];
            rng.fill(&mut v[..]);
            v
        });
        let proof = vec![i as u8; 128];
        let signature = [i as u8; 64];
        let ts = Timestamp::from_micros(g.max_timestamp + i as i64);
        if i % 2 == 0 {
            tx.insert_warrant(InsertWarrant {
                hash: &hash,
                author: &warrant_author,
                timestamp: ts,
                warrantee,
                proof: &proof,
                signature: &signature,
                reason: Some("bench"),
                storage_center_loc: warrantee.get_loc(),
                when_received: ts,
                when_integrated: ts,
                validation_status: 1,
                serialized_size: 256,
            })
            .await
            .expect("warrant");
        } else {
            tx.insert_limbo_warrant(InsertLimboWarrant {
                hash: &hash,
                author: &warrant_author,
                timestamp: ts,
                warrantee,
                proof: &proof,
                signature: &signature,
                reason: Some("bench"),
                storage_center_loc: warrantee.get_loc(),
                when_received: ts,
                serialized_size: 256,
            })
            .await
            .expect("limbo warrant");
        }
    }
    keys.warrantees = warrantees.into_iter().collect();

    tx.commit().await.expect("commit fixture");

    // Spread limbo ops over the three validation states and attempt counts.
    // `when_received` is random per op, so the modulo is a cheap uniform split.
    for sql in [
        "UPDATE LimboChainOp SET sys_validation_status = 1 WHERE when_received % 3 = 1",
        "UPDATE LimboChainOp SET sys_validation_status = 1, app_validation_status = 1 \
         WHERE when_received % 3 = 2",
        "UPDATE LimboChainOp SET sys_validation_attempts = when_received % 4, \
         app_validation_attempts = when_received % 3",
    ] {
        sqlx::query(sql)
            .execute(db.pool())
            .await
            .expect("limbo spread");
    }

    keys.outcome_integrated = sqlx::query_as::<_, (Vec<u8>, i64)>(
        "SELECT action_hash, op_type
         FROM ChainOp
         WHERE locally_validated = 1 AND validation_status = 1
         ORDER BY hash
         LIMIT ?",
    )
    .bind(SAMPLE as i64)
    .fetch_all(db.pool())
    .await
    .expect("integrated outcome samples")
    .into_iter()
    .map(|(action_hash, op_type)| (ActionHash::from_raw_36(action_hash), op_type))
    .collect();
    keys.outcome_limbo_decided = sqlx::query_as::<_, (Vec<u8>, i64)>(
        "SELECT action_hash, op_type
         FROM LimboChainOp
         WHERE sys_validation_status = 1
           AND app_validation_status = 1
           AND NOT EXISTS (
               SELECT 1
               FROM ChainOp
               WHERE ChainOp.action_hash = LimboChainOp.action_hash
                 AND ChainOp.op_type = LimboChainOp.op_type
           )
         ORDER BY hash
         LIMIT ?",
    )
    .bind(SAMPLE as i64)
    .fetch_all(db.pool())
    .await
    .expect("decided limbo outcome samples")
    .into_iter()
    .map(|(action_hash, op_type)| (ActionHash::from_raw_36(action_hash), op_type))
    .collect();
    keys.outcome_limbo_pending = sqlx::query_as::<_, (Vec<u8>, i64)>(
        "SELECT action_hash, op_type
         FROM LimboChainOp
         WHERE sys_validation_status IS NULL
           AND NOT EXISTS (
               SELECT 1
               FROM ChainOp
               WHERE ChainOp.action_hash = LimboChainOp.action_hash
                 AND ChainOp.op_type = LimboChainOp.op_type
           )
         ORDER BY hash
         LIMIT ?",
    )
    .bind(SAMPLE as i64)
    .fetch_all(db.pool())
    .await
    .expect("pending limbo outcome samples")
    .into_iter()
    .map(|(action_hash, op_type)| (ActionHash::from_raw_36(action_hash), op_type))
    .collect();

    for (action_hash, op_type) in &keys.outcome_integrated {
        let mut raw = action_hash.get_raw_36().to_vec();
        raw[0] ^= 0x80;
        let missing = ActionHash::from_raw_36(raw);
        let chain_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ChainOp WHERE action_hash = ? AND op_type = ?",
        )
        .bind(missing.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("missing ChainOp collision check");
        let limbo_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM LimboChainOp WHERE action_hash = ? AND op_type = ?",
        )
        .bind(missing.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("missing LimboChainOp collision check");
        assert_eq!(
            chain_count, 0,
            "missing outcome collides with ChainOp: {missing:?}/{op_type}"
        );
        assert_eq!(
            limbo_count, 0,
            "missing outcome collides with LimboChainOp: {missing:?}/{op_type}"
        );
        keys.outcome_missing.push((missing, *op_type));
    }

    for (action_hash, op_type) in &keys.outcome_integrated {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM ChainOp
             WHERE action_hash = ? AND op_type = ?
               AND locally_validated = 1 AND validation_status = 1",
        )
        .bind(action_hash.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("integrated outcome membership check");
        assert_eq!(
            count, 1,
            "invalid integrated outcome sample: {action_hash:?}/{op_type}"
        );
    }
    for (action_hash, op_type) in &keys.outcome_limbo_decided {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM LimboChainOp
             WHERE action_hash = ? AND op_type = ?
               AND sys_validation_status = 1 AND app_validation_status = 1",
        )
        .bind(action_hash.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("decided limbo outcome membership check");
        assert_eq!(
            count, 1,
            "invalid decided limbo sample: {action_hash:?}/{op_type}"
        );
        let integrated_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ChainOp WHERE action_hash = ? AND op_type = ?",
        )
        .bind(action_hash.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("decided limbo integrated exclusion check");
        assert_eq!(
            integrated_count, 0,
            "decided limbo sample is also integrated: {action_hash:?}/{op_type}"
        );
    }
    for (action_hash, op_type) in &keys.outcome_limbo_pending {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM LimboChainOp
             WHERE action_hash = ? AND op_type = ? AND sys_validation_status IS NULL",
        )
        .bind(action_hash.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("pending limbo outcome membership check");
        assert_eq!(
            count, 1,
            "invalid pending limbo sample: {action_hash:?}/{op_type}"
        );
        let integrated_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ChainOp WHERE action_hash = ? AND op_type = ?",
        )
        .bind(action_hash.get_raw_36())
        .bind(*op_type)
        .fetch_one(db.pool())
        .await
        .expect("pending limbo integrated exclusion check");
        assert_eq!(
            integrated_count, 0,
            "pending limbo sample is also integrated: {action_hash:?}/{op_type}"
        );
    }

    let (actions,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM Action")
        .fetch_one(db.pool())
        .await
        .expect("count actions");
    assert!(
        actions as usize >= cfg.actions * 95 / 100,
        "actions written: {actions}"
    );
    if cfg.actions >= 100_000 {
        assert!(
            keys.limbo_rows > 10_000,
            "100k fixture must exceed the 10k limbo LIMIT, has {}",
            keys.limbo_rows
        );
    }
    assert!(!keys.op_hashes.is_empty() && !keys.link_bases.is_empty());

    for (name, samples, expected) in [
        ("integrated", &keys.outcome_integrated, Some(1)),
        ("limbo_decided", &keys.outcome_limbo_decided, Some(1)),
        ("limbo_pending", &keys.outcome_limbo_pending, None),
        ("missing", &keys.outcome_missing, None),
    ] {
        assert!(!samples.is_empty(), "empty outcome cohort: {name}");
        for (action_hash, op_type) in samples {
            let actual = db
                .as_ref()
                .op_validation_outcome(action_hash, *op_type)
                .await
                .expect("outcome cohort lookup");
            assert_eq!(actual, expected, "incorrect outcome cohort: {name}");
        }
    }

    apply_extra_sql(&db).await;

    Fixture {
        size: cfg.actions,
        db,
        keys,
        _dir: dir,
    }
}

/// Apply `HC_DATA_BENCH_EXTRA_SQL` (one statement per line or `;`-separated)
/// after the fixture is built. Used to try candidate indexes.
async fn apply_extra_sql(db: &DbWrite<Dht>) {
    let Ok(path) = std::env::var("HC_DATA_BENCH_EXTRA_SQL") else {
        return;
    };
    let text = std::fs::read_to_string(&path).expect("read HC_DATA_BENCH_EXTRA_SQL");
    for stmt in text.split(';') {
        let stmt = stmt
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        if stmt.trim().is_empty() {
            continue;
        }
        let stmt_for_error = stmt.clone();
        sqlx::query(sqlx::AssertSqlSafe(stmt))
            .execute(db.pool())
            .await
            .unwrap_or_else(|e| panic!("extra sql failed: {e}\n{stmt_for_error}"));
    }
    eprintln!("applied extra SQL from {path}");
}
