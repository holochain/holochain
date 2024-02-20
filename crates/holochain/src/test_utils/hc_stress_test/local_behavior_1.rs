use super::*;

const B1_SYNC_TIME: std::time::Duration = std::time::Duration::from_secs(120);

struct Loc1PubData {
    pub_time: std::time::Instant,
    author: usize,
    found_shallow: std::collections::HashSet<usize>,
    found_full: std::collections::HashSet<usize>,
}

/// Local Behavior 1
/// - small means 32 bytes to 1024 bytes
/// - large means 1 MiB to 3 MiB
/// - starts conductors in sequence on a 2 s interval:
///   - 1 null node that shuts down after ~30 s
///   - 1 full query large publisher, pub every ~5 m, query every ~15 s
///   - 1 full query small publisher, pub every ~1 m, query every ~15 s
///   - 1 shallow query small publisher, pub every ~1 m, query every ~15 s
///   - 1 small single publisher that shuts down after ~3 m
///   - 1 large single publisher that shuts down after ~3 m
/// - starts additional nodes on 1 m intervals:
///   - shallow query only node that shuts down after ~3 m, query every ~15 s
pub struct LocalBehavior1 {
    runner: Option<HcStressTestRunner<Self>>,

    start_at: std::time::Instant,

    large_publish_count: usize,
    small_publish_count: usize,

    shallow_found_in_time: usize,
    shallow_found_later: usize,

    full_found_in_time: usize,
    full_found_later: usize,

    // this should only include nodes that run shallow or full queries
    shallow_validate_nodes: std::collections::HashMap<usize, std::time::Instant>,
    // this should only include nodes that run full queries
    full_validate_nodes: std::collections::HashMap<usize, std::time::Instant>,

    pub_data: std::collections::HashMap<ActionHash, Loc1PubData>,
}

impl std::fmt::Debug for LocalBehavior1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalBehavior1")
            .field("runtime", &self.start_at.elapsed())
            .field("large_publish_count", &self.large_publish_count)
            .field("small_publish_count", &self.small_publish_count)
            .field("shallow_found_in_time", &self.shallow_found_in_time)
            .field("shallow_found_later", &self.shallow_found_later)
            .field("full_found_in_time", &self.full_found_in_time)
            .field("full_found_later", &self.full_found_later)
            .finish()
    }
}

impl Report for LocalBehavior1 {
    fn spawn(&mut self, _node_id: usize) {}

    fn shutdown(&mut self, node_id: usize, _runtime: std::time::Duration) {
        self.validate();
        self.shallow_validate_nodes.remove(&node_id);
        self.full_validate_nodes.remove(&node_id);
    }

    fn publish(
        &mut self,
        node_id: usize,
        _runtime: std::time::Duration,
        byte_count: usize,
        hash: ActionHash,
    ) {
        if byte_count > 1024 {
            self.large_publish_count += 1;
        } else {
            self.small_publish_count += 1;
        }
        self.pub_data.insert(
            hash,
            Loc1PubData {
                pub_time: std::time::Instant::now(),
                author: node_id,
                found_shallow: std::collections::HashSet::new(),
                found_full: std::collections::HashSet::new(),
            },
        );
    }

    fn fetch_shallow(
        &mut self,
        node_id: usize,
        _runtime: std::time::Duration,
        hash_list: Vec<ActionHash>,
    ) {
        for hash in hash_list {
            if let Some(pub_data) = self.pub_data.get_mut(&hash) {
                if node_id != pub_data.author && pub_data.found_shallow.insert(node_id) {
                    if pub_data.pub_time.elapsed() > B1_SYNC_TIME {
                        self.shallow_found_later += 1;
                    } else {
                        self.shallow_found_in_time += 1;
                    }
                }
            }
        }
    }

    fn fetch_full(&mut self, node_id: usize, _runtime: std::time::Duration, hash: ActionHash) {
        if let Some(pub_data) = self.pub_data.get_mut(&hash) {
            if node_id != pub_data.author && pub_data.found_full.insert(node_id) {
                if pub_data.pub_time.elapsed() > B1_SYNC_TIME {
                    self.full_found_later += 1;
                } else {
                    self.full_found_in_time += 1;
                }
            }
        }
    }
}

impl LocalBehavior1 {
    /// LocalBehavior1 Constructor.
    pub fn new() -> Arc<Mutex<Self>> {
        let this = Arc::new(Mutex::new(Self {
            runner: None,
            start_at: std::time::Instant::now(),
            large_publish_count: 0,
            small_publish_count: 0,
            shallow_found_in_time: 0,
            shallow_found_later: 0,
            full_found_in_time: 0,
            full_found_later: 0,
            shallow_validate_nodes: std::collections::HashMap::new(),
            full_validate_nodes: std::collections::HashMap::new(),
            pub_data: std::collections::HashMap::new(),
        }));

        let runner = HcStressTestRunner::new(this.clone());
        this.lock().unwrap().runner = Some(runner);

        {
            let this = this.clone();
            tokio::task::spawn(async move {
                let network_seed = random_network_seed();
                let rendezvous = SweetLocalRendezvous::new().await;

                println!("spawn 1 null node that shuts down after ~30 s");
                let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                this.lock().unwrap().runner.as_ref().unwrap().add_node(
                    node,
                    SHUTDOWN_30_S,
                    vec![(0, BehaviorPublish::None)],
                    vec![(0, BehaviorQuery::None)],
                );

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                println!("spawn 1 full query large publisher, pub every ~5 m, query every ~15 s");
                let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                let node_id = this.lock().unwrap().runner.as_ref().unwrap().add_node(
                    node,
                    BehaviorLifetime::Forever,
                    vec![(0, PUBLISH_LARGE_5_M)],
                    vec![(0, QUERY_FULL_15_S)],
                );
                this.lock()
                    .unwrap()
                    .shallow_validate_nodes
                    .insert(node_id, std::time::Instant::now());
                this.lock()
                    .unwrap()
                    .full_validate_nodes
                    .insert(node_id, std::time::Instant::now());

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                println!("spawn 1 full query small publisher, pub every ~1 m, query every ~15 s");
                let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                let node_id = this.lock().unwrap().runner.as_ref().unwrap().add_node(
                    node,
                    BehaviorLifetime::Forever,
                    vec![(0, PUBLISH_SMALL_1_M)],
                    vec![(0, QUERY_FULL_15_S)],
                );
                this.lock()
                    .unwrap()
                    .shallow_validate_nodes
                    .insert(node_id, std::time::Instant::now());
                this.lock()
                    .unwrap()
                    .full_validate_nodes
                    .insert(node_id, std::time::Instant::now());

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                println!(
                    "spawn 1 shallow query small publisher, pub every ~1 m, query every ~15 s"
                );
                let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                let node_id = this.lock().unwrap().runner.as_ref().unwrap().add_node(
                    node,
                    BehaviorLifetime::Forever,
                    vec![(0, PUBLISH_SMALL_1_M)],
                    vec![(0, QUERY_SHALLOW_15_S)],
                );
                this.lock()
                    .unwrap()
                    .shallow_validate_nodes
                    .insert(node_id, std::time::Instant::now());

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                println!("spawn 1 small single publisher that shuts down after ~3 m");
                let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                this.lock().unwrap().runner.as_ref().unwrap().add_node(
                    node,
                    SHUTDOWN_3_M,
                    vec![(0, PUBLISH_SMALL_SINGLE)],
                    vec![(0, BehaviorQuery::None)],
                );

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                println!("spawn 1 large single publisher that shuts down after ~3 m");
                let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                this.lock().unwrap().runner.as_ref().unwrap().add_node(
                    node,
                    SHUTDOWN_3_M,
                    vec![(0, PUBLISH_LARGE_SINGLE)],
                    vec![(0, BehaviorQuery::None)],
                );

                loop {
                    for _ in 0..6 {
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        this.lock().unwrap().validate();
                    }

                    println!("spawn shallow query only node that shuts down after ~3 m, query every ~15 s");
                    let node = loc_test_conductor(network_seed.clone(), rendezvous.clone()).await;
                    let node_id = this.lock().unwrap().runner.as_ref().unwrap().add_node(
                        node,
                        SHUTDOWN_3_M,
                        vec![(0, BehaviorPublish::None)],
                        vec![(0, QUERY_SHALLOW_15_S)],
                    );
                    this.lock()
                        .unwrap()
                        .shallow_validate_nodes
                        .insert(node_id, std::time::Instant::now());
                }
            });
        }

        this
    }

    fn validate(&mut self) {
        // first make sure everything that has been published
        // has been seen shalowly by at least one non-authoring node
        for (_, pub_data) in self.pub_data.iter() {
            if pub_data.pub_time.elapsed() > B1_SYNC_TIME
                && (pub_data.found_shallow.is_empty() || pub_data.found_full.is_empty())
            {
                panic!("published item was not queried by anyone in {B1_SYNC_TIME:?}!");
            }
        }

        // now go through and ensure all querying nodes
        // can see everything they didn't publish,
        // so long as they have been online for at least sync time

        for (node_id, online) in self.shallow_validate_nodes.iter() {
            if online.elapsed() < B1_SYNC_TIME {
                continue;
            }

            for (hash, pub_data) in self.pub_data.iter() {
                if pub_data.pub_time.elapsed() > B1_SYNC_TIME
                    && pub_data.author != *node_id
                    && !pub_data.found_shallow.contains(node_id)
                {
                    panic!(
                        "node {node_id} could not shallow discover {hash} within {B1_SYNC_TIME:?}"
                    );
                }
            }
        }

        for (node_id, online) in self.full_validate_nodes.iter() {
            if online.elapsed() < B1_SYNC_TIME {
                continue;
            }

            for (hash, pub_data) in self.pub_data.iter() {
                if pub_data.pub_time.elapsed() > B1_SYNC_TIME
                    && pub_data.author != *node_id
                    && !pub_data.found_full.contains(node_id)
                {
                    panic!("node {node_id} could not full get {hash} within {B1_SYNC_TIME:?}");
                }
            }
        }
    }
}

async fn loc_test_conductor(network_seed: String, rendezvous: DynSweetRendezvous) -> HcStressTest {
    let config = SweetConductorConfig::rendezvous(true);
    let conductor = SweetConductor::from_config_rendezvous(config, rendezvous).await;
    let dna = HcStressTest::test_dna(network_seed).await;
    HcStressTest::new(conductor, &[dna]).await
}
