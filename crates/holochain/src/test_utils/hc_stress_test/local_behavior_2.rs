use super::*;

/// LocalBehavior2 was largely specified by the Holo team.
/// The target is 10 DNAs each run by 70 nodes for 3 weeks with
/// initial entry / link creation and occasional bursts of
/// additional entry / link creation.
/// Then only a handful of the dnas making any requests
/// roughly every 5 minutes.
/// We can't really run 700 dnas on one local system,
/// so this behavior will allow configuring that amount.
pub struct LocalBehavior2 {
    runner: Option<HcStressTestRunner<Self>>,

    start_at: std::time::Instant,

    total_publish_count: usize,
    total_shallow_fetch_count: usize,
    total_full_fetch_count: usize,
}

impl std::fmt::Debug for LocalBehavior2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalBehavior2")
            .field("runtime", &self.start_at.elapsed())
            .field("total_publish_count", &self.total_publish_count)
            .field("total_shallow_fetch_count", &self.total_shallow_fetch_count)
            .field("total_full_fetch_count", &self.total_full_fetch_count)
            .finish()
    }
}

impl Report for LocalBehavior2 {
    fn spawn(&mut self, _node_id: usize) {}

    fn shutdown(&mut self, _node_id: usize, _runtime: std::time::Duration) {}

    fn publish(
        &mut self,
        _node_id: usize,
        _runtime: std::time::Duration,
        _byte_count: usize,
        _hash: ActionHash,
    ) {
        self.total_publish_count += 1;
    }

    fn fetch_shallow(
        &mut self,
        _node_id: usize,
        _runtime: std::time::Duration,
        hash_list: Vec<ActionHash>,
    ) {
        self.total_shallow_fetch_count += hash_list.len();
    }

    fn fetch_full(&mut self, _node_id: usize, _runtime: std::time::Duration, _hash: ActionHash) {
        self.total_full_fetch_count += 1;
    }
}

impl LocalBehavior2 {
    /// LocalBehavior2 Constructor.
    pub fn new(dna_count: u8, node_count: u8) -> Arc<Mutex<Self>> {
        let this = Arc::new(Mutex::new(Self {
            runner: None,
            start_at: std::time::Instant::now(),
            total_publish_count: 0,
            total_shallow_fetch_count: 0,
            total_full_fetch_count: 0,
        }));

        let runner = HcStressTestRunner::new(this.clone());
        this.lock().unwrap().runner = Some(runner);

        {
            let this = this.clone();

            tokio::task::spawn(async move {
                let mut dna_files = Vec::new();
                for _ in 0..dna_count {
                    dna_files.push(HcStressTest::test_dna(random_network_seed()).await);
                }

                let rendezvous = SweetLocalRendezvous::new().await;

                for i in 0..node_count {
                    println!("spawn node {}/{node_count} with {dna_count} DNAs", i + 1,);

                    let node = loc_test_conductor(&dna_files, rendezvous.clone()).await;

                    let mut pub_behavior = Vec::new();
                    let mut query_behavior = Vec::new();

                    for cell in 0..dna_count {
                        if cell == 0 {
                            pub_behavior.push((
                                cell,
                                BehaviorPublish::Publish {
                                    byte_count_min: 1024,
                                    byte_count_max: 4096,
                                    publish_count: None,
                                    wait_min: std::time::Duration::from_secs(60 * 60 * 24 * 2),
                                    wait_max: std::time::Duration::from_secs(60 * 60 * 24 * 3),
                                },
                            ));
                        } else {
                            pub_behavior.push((
                                cell,
                                BehaviorPublish::Publish {
                                    byte_count_min: 1024,
                                    byte_count_max: 4096,
                                    publish_count: Some(1),
                                    wait_min: std::time::Duration::from_secs(20),
                                    wait_max: std::time::Duration::from_secs(60),
                                },
                            ));
                        }
                        query_behavior.push((
                            cell,
                            BehaviorQuery::Full {
                                wait_min: std::time::Duration::from_secs(60 * 4),
                                wait_max: std::time::Duration::from_secs(60 * 6),
                            },
                        ));
                    }

                    this.lock().unwrap().runner.as_ref().unwrap().add_node(
                        node,
                        BehaviorLifetime::Forever,
                        pub_behavior,
                        query_behavior,
                    );

                    // take some time to start up,
                    // booting holochain is very CPU intensive.
                    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                }
            });
        }

        this
    }
}

async fn loc_test_conductor(dna_files: &[DnaFile], rendezvous: DynSweetRendezvous) -> HcStressTest {
    let config = SweetConductorConfig::rendezvous(true);
    let conductor = SweetConductor::from_config_rendezvous(config, rendezvous).await;
    HcStressTest::new(conductor, dna_files).await
}
