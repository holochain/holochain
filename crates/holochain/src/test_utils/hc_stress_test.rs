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
        publish: BehaviorPublish,
        query: BehaviorQuery,
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

            let (mut publish_at, mut publish_count, byte_count_min, byte_count_max) = match publish
            {
                BehaviorPublish::None => (now.checked_add(MAX).unwrap(), Some(0), 0, 0),
                BehaviorPublish::Publish {
                    publish_count,
                    byte_count_min,
                    byte_count_max,
                    ..
                } => (now, publish_count, byte_count_min, byte_count_max),
            };

            let mut query_at = match query {
                BehaviorQuery::None => now.checked_add(MAX).unwrap(),
                _ => now,
            };

            loop {
                now = std::time::Instant::now();

                if now >= shutdown_at {
                    break;
                }

                if now >= publish_at {
                    publish_at = match publish {
                        BehaviorPublish::None => unreachable!(),
                        BehaviorPublish::Publish {
                            wait_min, wait_max, ..
                        } => now
                            .checked_add(rand::thread_rng().gen_range(wait_min..=wait_max))
                            .unwrap(),
                    };

                    let should_publish = {
                        match &mut publish_count {
                            Some(cnt) => {
                                if *cnt == 0 {
                                    publish_at = now.checked_add(MAX).unwrap();
                                    false
                                } else {
                                    *cnt -= 1;
                                    true
                                }
                            }
                            None => true,
                        }
                    };

                    if should_publish {
                        let bytes = {
                            let mut rng = rand::thread_rng();
                            let count = rng.gen_range(byte_count_min..=byte_count_max);
                            rand_utf8::rand_utf8(&mut rng, count)
                        };

                        let rec = node.create_file(&bytes).await;
                        let hash = HcStressTest::record_to_action_hash(&rec);

                        report.lock().unwrap().publish(
                            node_id,
                            init_time.elapsed(),
                            bytes.len(),
                            hash,
                        );
                    }
                }

                now = std::time::Instant::now();

                if now >= query_at {
                    query_at = match query {
                        BehaviorQuery::None => unreachable!(),
                        BehaviorQuery::Shallow {
                            wait_min, wait_max, ..
                        }
                        | BehaviorQuery::Full {
                            wait_min, wait_max, ..
                        } => now
                            .checked_add(rand::thread_rng().gen_range(wait_min..=wait_max))
                            .unwrap(),
                    };

                    let shallow_list = node.get_all_images().await;

                    report.lock().unwrap().fetch_shallow(
                        node_id,
                        init_time.elapsed(),
                        shallow_list.clone(),
                    );

                    if matches!(query, BehaviorQuery::Full { .. }) {
                        for hash in shallow_list {
                            if let Some(rec) = node.get_file(hash).await {
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

                now = std::time::Instant::now();

                let wait_dur = std::cmp::min(
                    shutdown_at.saturating_duration_since(now),
                    std::cmp::min(
                        publish_at.saturating_duration_since(now),
                        query_at.saturating_duration_since(now),
                    ),
                );

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
    cell: SweetCell,
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
    pub async fn new(mut conductor: SweetConductor, dna: DnaFile) -> Self {
        let app = conductor.setup_app("app", &[dna]).await.unwrap();
        let mut cells = app.into_cells();

        Self {
            conductor: Some(conductor),
            cell: cells.remove(0),
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
    pub async fn create_file(&mut self, data: &str) -> Record {
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
                &self.cell.zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "create_file",
                F {
                    data: data.as_bytes(),
                    uid: uid(),
                },
            )
            .await
    }

    /// Call the `get_all_images` zome function.
    pub async fn get_all_images(&mut self) -> Vec<ActionHash> {
        self.conductor
            .as_ref()
            .unwrap()
            .call(
                &self.cell.zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "get_all_images",
                (),
            )
            .await
    }

    /// Call the `get_file` zome function.
    pub async fn get_file(&mut self, hash: ActionHash) -> Option<Record> {
        self.conductor
            .as_ref()
            .unwrap()
            .call_fallible(
                &self.cell.zome(TestCoordinatorWasm::HcStressTestCoordinator),
                "get_file",
                hash,
            )
            .await
            .ok()
    }
}

mod local_behavior_1;
pub use local_behavior_1::*;
