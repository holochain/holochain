//! Measures how the cost of `must_get_agent_activity` grows with the length of
//! the author's source chain and with the size of the validator's DHT store.
//!
//! Context: <https://github.com/holochain/holochain/issues/6007>. The store
//! query behind `must_get_agent_activity` has no lower sequence bound on the
//! `Take(n)` path, so asking for the last `n` actions reads the author's whole
//! chain, and the query itself has no usable index, so every call scans every
//! `ChainOp` row in the store.
//!
//! The integrity zome mirrors how the Unyt DNA (`rave_engine::helper_fn`) uses
//! the host function from validation. For every app entry create it does:
//!
//! - a to-genesis walk from the action's predecessor
//!   (`get_agent_app_entries_activity(action, exclude_latest = true)`),
//! - an until-hash read of one source action
//!   (`read_source_activity`), and
//! - a `take(n)` read from the action
//!   (the rhai engine's `walk_on`, which carries a memoised walk forward), and
//! - the same `n` actions read as `until_hash(action, head)`, the DNA-side
//!   workaround for the unbounded `take`.
//!
//! Each call is timed inside validation on every node, and the raw store path
//! is timed directly at a peer at each checkpoint. A third agent then writes
//! filler entries so the store grows while the measured chain does not.
//!
//! This is a measurement, not a pass/fail regression test: it asserts only
//! that every walk eventually succeeds and prints the timings. Run it with
//!
//! ```text
//! MGAA_CHAIN_LENGTH=2000 MGAA_STEP=250 cargo nextest run -p holochain \
//!   --features slow_tests,build_wasms,encryption,wasmer-sys-cranelift \
//!   --test integration must_get_agent_activity_chain_growth \
//!   --run-ignored all --no-capture
//! ```

use holochain::prelude::*;
use holochain::sweettest::*;
use holochain_state::dht_store::DhtStoreRead;
use holochain_types::inline_zome::InlineZomeSet;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The three `ChainFilter` shapes the Unyt DNA uses from validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Walk {
    /// `ChainFilter::new(prev_action)`: the whole chain below the action.
    ToGenesis,
    /// `ChainFilter::until_hash(source, source)`: one action.
    UntilHash,
    /// `ChainFilter::take(action, n)`: the last `n` actions.
    Take,
    /// `ChainFilter::until_hash(action, head)`: the same `n` actions asked
    /// for by hash instead of by count, the DNA-side workaround for the
    /// unbounded `take`.
    TakeAsUntilHash,
}

impl Walk {
    const ALL: [Walk; 4] = [
        Walk::ToGenesis,
        Walk::UntilHash,
        Walk::Take,
        Walk::TakeAsUntilHash,
    ];

    fn name(self) -> &'static str {
        match self {
            Walk::ToGenesis => "to_genesis",
            Walk::UntilHash => "until_hash",
            Walk::Take => "take",
            Walk::TakeAsUntilHash => "take_as_uh",
        }
    }
}

/// How many actions the `take` walk asks for. Unyt's `walk_on` asks for the
/// gap since its memoised head, usually a handful of actions.
const TAKE_N: u32 = 2;

/// One timed `must_get_agent_activity` call made from inside validation.
#[derive(Clone, Debug)]
struct Sample {
    author: AgentPubKey,
    seq: u32,
    walk: Walk,
    elapsed: Duration,
    /// Number of actions returned, or the host error.
    outcome: Result<usize, String>,
}

type Sink = Arc<Mutex<Vec<Sample>>>;

/// A store-path checkpoint: label, alice's chain top seq, bob's op count, and
/// per walk the timing stats and the number of actions returned.
type StoreRow = (String, u32, u64, BTreeMap<Walk, (Stats, usize)>);

/// A validation checkpoint: label, node, and per walk the timing stats and
/// the range of chain sequences validated.
type ValidationRow = (String, &'static str, BTreeMap<Walk, (Stats, (u32, u32))>);

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
struct Thing(u32);

/// The coordinator zome shared by every node. Cloning this keeps the inline
/// zome id, and so the DNA hash, identical across nodes.
fn base_zomes() -> SweetInlineZomes {
    SweetInlineZomes::new(vec![EntryDef::default_from_id("thing")], 0).function(
        "create_thing",
        |api, n: u32| {
            let entry = Entry::app(Thing(n).try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        },
    )
}

/// The zomes for one node: the shared base plus a validate callback that
/// reports its timings into `sink`, so timings are attributed per node.
fn zomes(base: &SweetInlineZomes, sink: Sink) -> SweetInlineZomes {
    base.clone()
        .integrity_function("validate", move |api, op: Op| {
            // Unyt walks the chain when validating StoreEntry and StoreRecord
            // ops of its app entries.
            let is_app_create = op.action_type() == ActionType::Create
                && matches!(op.entry_data(), Some((_, EntryType::App(_))));
            if !is_app_create || !matches!(op, Op::CreateEntry(_) | Op::CreateRecord(_)) {
                return Ok(ValidateCallbackResult::Valid);
            }
            let author = op.author().clone();
            let seq = op.action_seq();
            let top = op.action_hash().clone();
            let prev = op
                .prev_action()
                .cloned()
                .expect("an app entry create always has a predecessor");

            for walk in Walk::ALL {
                let chain_filter = match walk {
                    Walk::ToGenesis => ChainFilter::new(prev.clone()),
                    Walk::UntilHash => ChainFilter::until_hash(prev.clone(), prev.clone()),
                    Walk::Take => ChainFilter::take(top.clone(), TAKE_N),
                    Walk::TakeAsUntilHash => ChainFilter::until_hash(top.clone(), prev.clone()),
                };
                let input = MustGetAgentActivityInput {
                    author: author.clone(),
                    chain_filter: chain_filter.clone(),
                };
                let start = Instant::now();
                let result = api.must_get_agent_activity(input);
                let elapsed = start.elapsed();
                sink.lock().unwrap().push(Sample {
                    author: author.clone(),
                    seq,
                    walk,
                    elapsed,
                    outcome: result.as_ref().map(Vec::len).map_err(|e| e.to_string()),
                });
                if result.is_err() {
                    // A WASM zome gets this short-circuited by the host; an
                    // inline zome has to say it itself so the op is retried
                    // once the chain is held.
                    return Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(author, chain_filter),
                    ));
                }
            }
            Ok(ValidateCallbackResult::Valid)
        })
}

/// Summary of a set of durations.
#[derive(Clone, Copy, Debug, Default)]
struct Stats {
    n: usize,
    errors: usize,
    min: Duration,
    median: Duration,
    mean: Duration,
    max: Duration,
}

impl Stats {
    fn of(mut durations: Vec<Duration>, errors: usize) -> Self {
        if durations.is_empty() {
            return Self {
                errors,
                ..Default::default()
            };
        }
        durations.sort();
        let n = durations.len();
        let sum: Duration = durations.iter().sum();
        Self {
            n,
            errors,
            min: durations[0],
            median: durations[n / 2],
            mean: sum / n as u32,
            max: durations[n - 1],
        }
    }
}

fn ms(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64() * 1000.0)
}

/// Validation samples grouped by walk for one author: the stats and the
/// range of chain sequences the samples validated.
fn summarise(samples: &[Sample], author: &AgentPubKey) -> BTreeMap<Walk, (Stats, (u32, u32))> {
    let mut out = BTreeMap::new();
    for walk in Walk::ALL {
        let mine: Vec<&Sample> = samples
            .iter()
            .filter(|s| s.walk == walk && &s.author == author)
            .collect();
        let errors = mine.iter().filter(|s| s.outcome.is_err()).count();
        let ok: Vec<Duration> = mine
            .iter()
            .filter(|s| s.outcome.is_ok())
            .map(|s| s.elapsed)
            .collect();
        let lo = mine.iter().map(|s| s.seq).min().unwrap_or(0);
        let hi = mine.iter().map(|s| s.seq).max().unwrap_or(0);
        out.insert(walk, (Stats::of(ok, errors), (lo, hi)));
    }
    out
}

/// Time the raw store path (no scratch, no network) for each walk shape,
/// `reps` times each, against the chain ending at `top`, whose predecessor
/// is `prev`.
async fn time_store(
    store: &DhtStoreRead,
    author: &AgentPubKey,
    top: &ActionHash,
    prev: &ActionHash,
    reps: usize,
) -> BTreeMap<Walk, (Stats, usize)> {
    let mut out = BTreeMap::new();
    for walk in Walk::ALL {
        let filter = match walk {
            Walk::ToGenesis => ChainFilter::new(top.clone()),
            Walk::UntilHash => ChainFilter::until_hash(top.clone(), top.clone()),
            Walk::Take => ChainFilter::take(top.clone(), TAKE_N),
            Walk::TakeAsUntilHash => ChainFilter::until_hash(top.clone(), prev.clone()),
        };
        let mut durations = Vec::with_capacity(reps);
        let mut returned = 0;
        for _ in 0..reps {
            let start = Instant::now();
            let response = store
                .must_get_agent_activity(author, &filter)
                .await
                .unwrap();
            durations.push(start.elapsed());
            match response {
                MustGetAgentActivityResponse::Activity { activity, .. } => {
                    returned = activity.len();
                }
                other => panic!("store walk {walk:?} did not complete: {other:?}"),
            }
        }
        out.insert(walk, (Stats::of(durations, 0), returned));
    }
    out
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

async fn create_things(
    conductor: &SweetConductor,
    cell: &SweetCell,
    from: usize,
    to: usize,
) -> ActionHash {
    let mut last = None;
    for n in from..to {
        let hash: ActionHash = conductor
            .call(
                &cell.zome(SweetInlineZomes::COORDINATOR),
                "create_thing",
                n as u32,
            )
            .await;
        last = Some(hash);
    }
    last.expect("at least one entry created")
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[ignore = "performance measurement for holochain#6007, run explicitly with --run-ignored all --no-capture"]
async fn must_get_agent_activity_cost_grows_with_chain_and_store() {
    holochain_trace::test_run();

    let chain_length = env_usize("MGAA_CHAIN_LENGTH", 1000);
    let step = env_usize("MGAA_STEP", 200).max(1);
    let filler = env_usize("MGAA_FILLER", chain_length);
    let store_reps = env_usize("MGAA_STORE_REPS", 5).max(1);
    let consistency_timeout_s = env_usize("MGAA_CONSISTENCY_TIMEOUT_S", 600) as u64;

    const NODES: [&str; 3] = ["alice", "bob", "carol"];
    let sinks: Vec<Sink> = NODES.iter().map(|_| Sink::default()).collect();

    // One DNA per node, all with the same hash, so each node's timings land
    // in its own sink.
    let network_seed = format!(
        "mgaa-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let base = base_zomes();
    let mut dnas = Vec::new();
    for sink in &sinks {
        let (dna, _, _) =
            SweetDnaFile::from_inline_zomes(network_seed.clone(), zomes(&base, sink.clone())).await;
        dnas.push(dna);
    }
    assert!(dnas.iter().all(|d| d.dna_hash() == dnas[0].dna_hash()));

    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        NODES.len(),
        SweetConductorConfig::rendezvous(true),
    )
    .await
    .into_inner();
    let mut cells = Vec::new();
    for (conductor, dna) in conductors.iter_mut().zip(dnas) {
        let (cell,) = conductor
            .setup_app("app", [&dna])
            .await
            .unwrap()
            .into_tuple();
        cells.push(cell);
    }
    SweetConductor::exchange_peer_info(conductors.iter()).await;

    let alice = cells[0].agent_pubkey().clone();
    let carol = cells[2].agent_pubkey().clone();
    let bob_store = cells[1].dht_store().as_read();

    let mut store_rows: Vec<StoreRow> = Vec::new();
    let mut validation_rows: Vec<ValidationRow> = Vec::new();

    // The chain top's sequence and its predecessor, as held by bob.
    let top_seq = |top: &ActionHash, store: &DhtStoreRead| {
        let top = top.clone();
        let store = store.clone();
        let alice = alice.clone();
        async move {
            match store
                .must_get_agent_activity(&alice, &ChainFilter::take(top, 1))
                .await
                .unwrap()
            {
                MustGetAgentActivityResponse::Activity { activity, .. } => {
                    let action = activity[0].action.action();
                    (action.action_seq(), action.prev_action().cloned().unwrap())
                }
                other => panic!("chain top not held: {other:?}"),
            }
        }
    };

    let started = Instant::now();
    let mut created = 0;
    while created < chain_length {
        let target = (created + step).min(chain_length);
        let batch_size = target - created;
        let batch_started = Instant::now();
        let top = create_things(&conductors[0], &cells[0], created, target).await;
        let authored_in = batch_started.elapsed();
        created = target;

        let consistency_started = Instant::now();
        await_consistency_s(consistency_timeout_s, cells.iter())
            .await
            .unwrap();
        let consistent_in = consistency_started.elapsed();

        let (seq, prev) = top_seq(&top, &bob_store).await;
        let ops = bob_store.count_all_ops().await.unwrap();
        let label = format!("chain={created}");
        println!(
            "[{label}] alice chain top seq {seq}, bob holds {ops} ops; authored {batch_size} entries in {}s, consistent after {}s (elapsed {}s)",
            authored_in.as_secs(),
            consistent_in.as_secs(),
            started.elapsed().as_secs()
        );

        store_rows.push((
            label.clone(),
            seq,
            ops,
            time_store(&bob_store, &alice, &top, &prev, store_reps).await,
        ));
        for (node, sink) in NODES.iter().zip(&sinks) {
            let samples = std::mem::take(&mut *sink.lock().unwrap());
            validation_rows.push((label.clone(), node, summarise(&samples, &alice)));
        }
    }

    let alice_top = {
        // Re-read the top from bob's store rather than trusting the last hash.
        let seq_before = store_rows.last().unwrap().1;
        let top = create_things(&conductors[0], &cells[0], created, created + 1).await;
        await_consistency_s(consistency_timeout_s, cells.iter())
            .await
            .unwrap();
        let (seq, prev) = top_seq(&top, &bob_store).await;
        assert_eq!(seq, seq_before + 1);
        (top, prev)
    };

    // Phase 2: carol grows bob's store without touching alice's chain.
    if filler > 0 {
        let filler_started = Instant::now();
        create_things(&conductors[2], &cells[2], 0, filler).await;
        await_consistency_s(consistency_timeout_s, cells.iter())
            .await
            .unwrap();
        println!(
            "[filler] carol authored {filler} entries, consistent after {}s",
            filler_started.elapsed().as_secs()
        );
        let (seq, _) = top_seq(&alice_top.0, &bob_store).await;
        let ops = bob_store.count_all_ops().await.unwrap();
        let label = format!("chain={} +filler={filler}", created + 1);
        store_rows.push((
            label.clone(),
            seq,
            ops,
            time_store(&bob_store, &alice, &alice_top.0, &alice_top.1, store_reps).await,
        ));
        for (node, sink) in NODES.iter().zip(&sinks) {
            let samples = std::mem::take(&mut *sink.lock().unwrap());
            // Carol's entries are what was validated in this phase.
            validation_rows.push((label.clone(), node, summarise(&samples, &carol)));
        }
    }

    println!();
    println!("== Store path at bob (DhtStore::must_get_agent_activity on alice's chain), ms ==");
    println!(
        "{:<28} {:>8} {:>10} {:<11} {:>8} {:>9} {:>9} {:>9} {:>9}",
        "checkpoint", "top_seq", "bob_ops", "walk", "returned", "min", "median", "mean", "max"
    );
    for (label, seq, ops, by_walk) in &store_rows {
        for (walk, (stats, returned)) in by_walk {
            println!(
                "{:<28} {:>8} {:>10} {:<11} {:>8} {:>9} {:>9} {:>9} {:>9}",
                label,
                seq,
                ops,
                walk.name(),
                returned,
                ms(stats.min),
                ms(stats.median),
                ms(stats.mean),
                ms(stats.max)
            );
        }
    }

    println!();
    println!("== Inside validation (api.must_get_agent_activity), ms; alice validates her own chain inline, bob and carol validate it as peers ==");
    println!(
        "{:<28} {:<6} {:<11} {:>5} {:>6} {:>11} {:>9} {:>9} {:>9} {:>9}",
        "checkpoint", "node", "walk", "n", "errors", "seq_range", "min", "median", "mean", "max"
    );
    for (label, node, by_walk) in &validation_rows {
        for (walk, (stats, (lo, hi))) in by_walk {
            println!(
                "{:<28} {:<6} {:<11} {:>5} {:>6} {:>11} {:>9} {:>9} {:>9} {:>9}",
                label,
                node,
                walk.name(),
                stats.n,
                stats.errors,
                format!("{lo}-{hi}"),
                ms(stats.min),
                ms(stats.median),
                ms(stats.mean),
                ms(stats.max)
            );
        }
    }

    // Every checkpoint must have produced a complete `take` answer of the
    // requested size, so the numbers above describe successful walks.
    for (label, _, _, by_walk) in &store_rows {
        assert_eq!(
            by_walk[&Walk::Take].1,
            TAKE_N as usize,
            "take({TAKE_N}) at {label} did not return {TAKE_N} actions"
        );
        assert_eq!(
            by_walk[&Walk::UntilHash].1,
            1,
            "until_hash at {label} did not return exactly the source action"
        );
        assert_eq!(
            by_walk[&Walk::TakeAsUntilHash].1,
            TAKE_N as usize,
            "until_hash(top, prev) at {label} did not return {TAKE_N} actions"
        );
    }
}
