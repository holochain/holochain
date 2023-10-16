//! automated behavioral testing of hc-stress-test zomes

use crate::sweettest::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::*;
use std::sync::{atomic, Arc, Mutex};

const MAX: std::time::Duration = std::time::Duration::from_secs(60 * 60 * 24 * 365 * 10);

/// Define the lifetime behavior of a test node.
#[derive(Debug, Clone, Copy)]
pub enum BehaviorLifetime {
    /// The node will continue to exist.
    Forever,

    /// The node will shutdown and be dropped after the given duration.
    Shutdown {
        /// The minimum time to wait before shutdown.
        wait_min: std::time::Duration,

        /// The maximum time to wait before shutdown.
        wait_max: std::time::Duration,
    },
}

const SHUTDOWN_30_S: BehaviorLifetime = BehaviorLifetime::Shutdown {
    wait_min: std::time::Duration::from_secs(25),
    wait_max: std::time::Duration::from_secs(35),
};

const SHUTDOWN_3_M: BehaviorLifetime = BehaviorLifetime::Shutdown {
    wait_min: std::time::Duration::from_secs(160),
    wait_max: std::time::Duration::from_secs(200),
};

/// Define the publish behavior of a test node.
#[derive(Debug, Clone, Copy)]
pub enum BehaviorPublish {
    /// The node will never publish authored content.
    None,

    /// The node will publish authored content.
    Publish {
        /// The minimum size in bytes of content to be published.
        byte_count_min: usize,

        /// The maximum size in bytes of content to be published.
        byte_count_max: usize,

        /// The number of times this node will publish (None for infinite).
        publish_count: Option<usize>,

        /// The minimum time to wait between publishes.
        wait_min: std::time::Duration,

        /// The maximum time to wait between publishes.
        wait_max: std::time::Duration,
    },
}

const PUBLISH_LARGE_5_M: BehaviorPublish = BehaviorPublish::Publish {
    byte_count_min: 1024 * 1024,
    byte_count_max: 1024 * 1024 * 3,
    publish_count: None,
    wait_min: std::time::Duration::from_secs(30 * 9),
    wait_max: std::time::Duration::from_secs(30 * 11),
};

const PUBLISH_LARGE_SINGLE: BehaviorPublish = BehaviorPublish::Publish {
    byte_count_min: 1024 * 1024,
    byte_count_max: 1024 * 1024 * 3,
    publish_count: Some(1),
    wait_min: MAX,
    wait_max: MAX,
};

const PUBLISH_SMALL_1_M: BehaviorPublish = BehaviorPublish::Publish {
    byte_count_min: 32,
    byte_count_max: 1024,
    publish_count: None,
    wait_min: std::time::Duration::from_secs(50),
    wait_max: std::time::Duration::from_secs(70),
};

const PUBLISH_SMALL_SINGLE: BehaviorPublish = BehaviorPublish::Publish {
    byte_count_min: 32,
    byte_count_max: 1024,
    publish_count: Some(1),
    wait_min: MAX,
    wait_max: MAX,
};

/// Define the query behavior of a test node.
#[derive(Debug, Clone, Copy)]
pub enum BehaviorQuery {
    /// The node will never query data from other nodes.
    None,

    /// The node will fetch hashes only, but will never request content.
    Shallow {
        /// The minimum time to wait between queries.
        wait_min: std::time::Duration,

        /// The maximum time to wait between queries.
        wait_max: std::time::Duration,
    },

    /// The node will fetch hashes, then entry data.
    Full {
        /// The minimum time to wait between queries.
        wait_min: std::time::Duration,

        /// The maximum time to wait between queries.
        wait_max: std::time::Duration,
    },
}

const QUERY_SHALLOW_15_S: BehaviorQuery = BehaviorQuery::Shallow {
    wait_min: std::time::Duration::from_secs(13),
    wait_max: std::time::Duration::from_secs(17),
};

const QUERY_FULL_15_S: BehaviorQuery = BehaviorQuery::Full {
    wait_min: std::time::Duration::from_secs(13),
    wait_max: std::time::Duration::from_secs(17),
};

/// Report what happened with a behavior.
pub trait Report: 'static + Send {
    /// A node has been spawned into the runner.
    fn spawn(&mut self, node_id: usize);

    /// A node has shutdown and was removed from the runner.
    fn shutdown(&mut self, node_id: usize, runtime: std::time::Duration);

    /// Result of a publish attempt.
    fn publish(
        &mut self,
        node_id: usize,
        runtime: std::time::Duration,
        byte_count: usize,
        hash: ActionHash,
    );

    /// Result of a shallow fetch attempt.
    fn fetch_shallow(
        &mut self,
        node_id: usize,
        runtime: std::time::Duration,
        hash_list: Vec<ActionHash>,
    );

    /// Result of a full fetch attempt.
    fn fetch_full(&mut self, node_id: usize, runtime: std::time::Duration, hash: ActionHash);
}

/// Run an hc-stress-test behavior test.
pub struct HcStressTestRunner<R: Report>(Arc<Mutex<R>>);

impl<R: Report> HcStressTestRunner<R> {
    /// Construct a new runner instance.
    pub fn new(r: Arc<Mutex<R>>) -> Self {
        Self(r)
    }

    /// Add a node to the runner with given behavior.
    /// Returns the node_id that was added.
    pub fn add_node(
        &self,
        mut node: HcStressTest,
        lifetime: BehaviorLifetime,
        publish: Vec<(u8, BehaviorPublish)>,
        query: Vec<(u8, BehaviorQuery)>,
    ) -> usize {
        use rand::Rng;

        let report = self.0.clone();
        let init_time = std::time::Instant::now();

        static NODE_ID: atomic::AtomicUsize = atomic::AtomicUsize::new(1);
        let node_id = NODE_ID.fetch_add(1, atomic::Ordering::Relaxed);

        tokio::task::spawn(async move {
            struct OnDrop<R: Report>(Arc<Mutex<R>>, usize, std::time::Instant);
            impl<R: Report> Drop for OnDrop<R> {
                fn drop(&mut self) {
                    self.0.lock().unwrap().shutdown(self.1, self.2.elapsed());
                }
            }

            let _on_drop = OnDrop(report.clone(), node_id, init_time);

            report.lock().unwrap().spawn(node_id);

            let mut now = std::time::Instant::now();

            let shutdown_at = match lifetime {
                BehaviorLifetime::Forever => now.checked_add(MAX).unwrap(),
                BehaviorLifetime::Shutdown { wait_min, wait_max } => now
                    .checked_add(rand::thread_rng().gen_range(wait_min..=wait_max))
                    .unwrap(),
            };

            struct PubData {
                pub next_at: std::time::Instant,
                pub cell: u8,
                pub count: usize,
                pub bc_min: usize,
                pub bc_max: usize,
                pub w_min: std::time::Duration,
                pub w_max: std::time::Duration,
            }

            let mut next_publish_at = Vec::new();

            for p in &publish {
                next_publish_at.push(match p {
                    (cell, BehaviorPublish::None) => PubData {
                        next_at: now.checked_add(MAX).unwrap(),
                        cell: *cell,
                        count: 0,
                        bc_min: 0,
                        bc_max: 0,
                        w_min: std::time::Duration::MAX,
                        w_max: std::time::Duration::MAX,
                    },
                    (
                        cell,
                        BehaviorPublish::Publish {
                            byte_count_min,
                            byte_count_max,
                            publish_count,
                            wait_min,
                            wait_max,
                        },
                    ) => {
                        let count = match publish_count {
                            None => usize::MAX,
                            Some(c) => *c,
                        };
                        PubData {
                            next_at: now,
                            cell: *cell,
                            count,
                            bc_min: *byte_count_min,
                            bc_max: *byte_count_max,
                            w_min: *wait_min,
                            w_max: *wait_max,
                        }
                    }
                });
            }

            struct QueryData {
                pub next_at: std::time::Instant,
                pub cell: u8,
                pub is_full: bool,
                pub w_min: std::time::Duration,
                pub w_max: std::time::Duration,
            }

            let mut next_query_at = Vec::new();

            for q in &query {
                next_query_at.push(match q {
                    (cell, BehaviorQuery::None) => QueryData {
                        next_at: now.checked_add(MAX).unwrap(),
                        cell: *cell,
                        is_full: false,
                        w_min: std::time::Duration::MAX,
                        w_max: std::time::Duration::MAX,
                    },
                    (cell, BehaviorQuery::Shallow { wait_min, wait_max }) => QueryData {
                        next_at: now,
                        cell: *cell,
                        is_full: false,
                        w_min: *wait_min,
                        w_max: *wait_max,
                    },
                    (cell, BehaviorQuery::Full { wait_min, wait_max }) => QueryData {
                        next_at: now,
                        cell: *cell,
                        is_full: true,
                        w_min: *wait_min,
                        w_max: *wait_max,
                    },
                });
            }

            loop {
                now = std::time::Instant::now();

                if now >= shutdown_at {
                    break;
                }

                let mut next_check_at = shutdown_at;

                for p in &mut next_publish_at {
                    now = std::time::Instant::now();

                    let should_publish = if now >= p.next_at {
                        p.next_at = now
                            .checked_add(rand::thread_rng().gen_range(p.w_min..=p.w_max))
                            .unwrap();
                        if p.count > 0 {
                            p.count -= 1;
                            true
                        } else {
                            p.next_at = now.checked_add(MAX).unwrap();
                            false
                        }
                    } else {
                        false
                    };

                    if p.next_at < next_check_at {
                        next_check_at = p.next_at;
                    }

                    if should_publish {
                        let bytes = {
                            let mut rng = rand::thread_rng();
                            let count = rng.gen_range(p.bc_min..=p.bc_max);
                            rand_utf8::rand_utf8(&mut rng, count)
                        };

                        let rec = node.create_file(p.cell, &bytes).await;
                        let hash = HcStressTest::record_to_action_hash(&rec);

                        report.lock().unwrap().publish(
                            node_id,
                            init_time.elapsed(),
                            bytes.len(),
                            hash,
                        );
                    }
                }

                for q in &mut next_query_at {
                    now = std::time::Instant::now();

                    if now >= q.next_at {
                        q.next_at = now
                            .checked_add(rand::thread_rng().gen_range(q.w_min..=q.w_max))
                            .unwrap();

                        let shallow_list = node.get_all_images(q.cell).await;

                        report.lock().unwrap().fetch_shallow(
                            node_id,
                            init_time.elapsed(),
                            shallow_list.clone(),
                        );

                        if q.is_full {
                            for hash in shallow_list {
                                if let Some(rec) = node.get_file(q.cell, hash).await {
                                    let hash = HcStressTest::record_to_action_hash(&rec);
                                    report.lock().unwrap().fetch_full(
                                        node_id,
                                        init_time.elapsed(),
                                        hash,
                                    );
                                }
                            }
                        }
                    }

                    if q.next_at < next_check_at {
                        next_check_at = q.next_at;
                    }
                }

                now = std::time::Instant::now();

                let wait_dur = next_check_at.saturating_duration_since(now);

                tokio::time::sleep(wait_dur).await;
            }
        });

        node_id
    }
}

fn uid() -> i64 {
    use rand::Rng;
    rand::thread_rng().gen()
}

/// A conductor running the hc_stress_test app.
pub struct HcStressTest {
    conductor: Option<SweetConductor>,
    cells: Vec<SweetCell>,
}

impl Drop for HcStressTest {
    fn drop(&mut self) {
        if let Some(mut conductor) = self.conductor.take() {
            tokio::task::spawn(async move {
                // MAYBE: someday it'd be nice to know this conductor isn't
                //        phantom running in the background, but as it is
                //        we are ignoring the shutdown errors (which it
                //        totally generates). We mostly just care that it
                //        hasn't panicked any tokio task threads.
                let _ = conductor.try_shutdown().await;
            });
        }
    }
}

impl HcStressTest {
    /// Helper to provide the SweetDnaFile from compiled test wasms.
    pub async fn test_dna(network_seed: String) -> DnaFile {
        let (dna, _, _) = SweetDnaFile::from_zomes(
            network_seed,
            vec![TestIntegrityWasm::HcStressTestIntegrity],
            vec![TestCoordinatorWasm::HcStressTestCoordinator],
            vec![
                DnaWasm::from(TestIntegrityWasm::HcStressTestIntegrity),
                DnaWasm::from(TestCoordinatorWasm::HcStressTestCoordinator),
            ],
            SerializedBytes::default(),
        )
        .await;
        dna
    }

    /// Given a new/blank sweet conductor and the hc_stress_test dna
    /// (see [HcStressTest::test_dna]), install the dna, returning
    /// a conductor running the hc_stress_test app.
    pub async fn new(mut conductor: SweetConductor, dna_files: &[DnaFile]) -> Self {
        let app = conductor.setup_app("app", dna_files).await.unwrap();
        let cells = app.into_cells();

        Self {
            conductor: Some(conductor),
            cells,
        }
    }

    /// Extract the ActionHash from a Record.
    pub fn record_to_action_hash(record: &Record) -> ActionHash {
        record.signed_action.hashed.hash.clone()
    }

    /// Extract the file data from a Record.
    pub fn record_to_file_data(record: &Record) -> String {
        match record {
            Record {
                entry: RecordEntry::Present(Entry::App(AppEntryBytes(bytes))),
                ..
            } => {
                #[derive(Debug, serde::Deserialize)]
                struct F<'a> {
                    #[serde(with = "serde_bytes")]
                    data: &'a [u8],
                    #[allow(dead_code)]
                    uid: i64,
                }
                let f: F<'_> = decode(bytes.bytes()).unwrap();
                String::from_utf8_lossy(f.data).to_string()
            }
            _ => panic!("record does not contain file data"),
        }
    }

    /// Call the `create_file` zome function.
    pub async fn create_file(&mut self, cell: u8, data: &str) -> Record {
        #[derive(Debug, serde::Serialize)]
        struct F<'a> {
            #[serde(with = "serde_bytes")]
            data: &'a [u8],
            uid: i64,
        }
        self.conductor
            .as_ref()
            .unwrap()
            .call(
                &self.cells[cell as usize].zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "create_file",
                F {
                    data: data.as_bytes(),
                    uid: uid(),
                },
            )
            .await
    }

    /// Call the `get_all_images` zome function.
    pub async fn get_all_images(&mut self, cell: u8) -> Vec<ActionHash> {
        self.conductor
            .as_ref()
            .unwrap()
            .call(
                &self.cells[cell as usize].zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "get_all_images",
                (),
            )
            .await
    }

    /// Call the `get_file` zome function.
    pub async fn get_file(&mut self, cell: u8, hash: ActionHash) -> Option<Record> {
        self.conductor
            .as_ref()
            .unwrap()
            .call_fallible(
                &self.cells[cell as usize].zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "get_file",
                hash,
            )
            .await
            .ok()
    }
}

mod local_behavior_1;
pub use local_behavior_1::*;

mod local_behavior_2;
pub use local_behavior_2::*;
