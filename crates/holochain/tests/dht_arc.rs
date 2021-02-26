use holochain::test_utils::sweetest::SweetAgents;
use holochain::test_utils::sweetest::SweetConductor;
use holochain_keystore::KeystoreSender;
use holochain_p2p::dht_arc::check_for_gaps;
use holochain_p2p::dht_arc::MAX_HALF_LENGTH;
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::dht_arc::DhtArcBucket;
use kitsune_p2p::*;

async fn get_peers(num: usize, half_lens: &[u32], keystore: KeystoreSender) -> Vec<DhtArc> {
    let mut hl = half_lens.iter();
    let mut iter = std::iter::repeat_with(|| hl.next().unwrap_or(&half_lens[0]));
    let mut out = Vec::with_capacity(num);

    let agents = SweetAgents::get(keystore, num).await;
    for agent in agents {
        let agent = holochain_p2p::agent_holo_to_kit(agent);
        let arc = DhtArc::new(agent.get_loc(), *iter.next().unwrap());
        out.push(arc);
    }
    // show_dist(out.clone());
    out
}

fn _show_dist(mut peers: Vec<DhtArc>) {
    peers.sort_unstable_by_key(|a| a.center_loc.0);

    let div = 8;
    let size = u32::MAX / div;
    let mut out = String::new();
    let total = peers.len() as f64;

    let mut peers = peers.into_iter().peekable();

    for i in 1..(div + 1) {
        let range = (i - 1)..(size * i);
        let mut count = 0;
        loop {
            match peers.peek() {
                Some(p) => {
                    if range.contains(&p.center_loc.0 .0) {
                        peers.next().unwrap();
                        count += 1;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
        out.push_str(&format!(
            " {:.4}% ",
            // (i - 1) as f64 / div as f64,
            // i as f64 / div as f64,
            count as f64 / total * 100.0,
        ));
    }
    println!("{}", out);
}

#[tokio::test(threaded_scheduler)]
async fn test_arc_keys() {
    let conductor = SweetConductor::from_config(Default::default()).await;
    let keystore = conductor.keystore();
    let min_peers = 40;

    let converge = |peers: &mut Vec<DhtArc>| {
        for _ in 0..40 {
            // dbg!(i);
            for i in 0..peers.len() {
                // dbg!(i);
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let bucket = DhtArcBucket::new(*arc, p.clone());
                let density = bucket.density();
                arc.update_length(density);
                // let bucket = DhtArcBucket::new(*arc, p.clone());
                // println!("{}\n{:?}", bucket, bucket.density().est_gap());
                // println!("{}", bucket.density().est_gap());
            }
            let bucket = DhtArcBucket::new(peers[0], peers.clone());
            println!("{}\n{:?}", bucket, bucket.density().est_gap());
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    };

    let mut peers = get_peers(min_peers * 4, &[20], keystore.clone()).await;
    converge(&mut peers);
    // - Converge to half coverage
    for arc in peers {
        assert_eq!((arc.coverage() * 10.0).round() / 10.0, 0.25);
    }

    let mut peers = get_peers(min_peers, &[20], keystore.clone()).await;
    converge(&mut peers);
    // - Converge to half coverage
    for arc in peers {
        assert_eq!((arc.coverage() * 10.0).round() / 10.0, 0.5);
    }

    let mut peers = get_peers(min_peers * 2, &[20], keystore.clone()).await;
    converge(&mut peers);
    // - Converge to half coverage
    for arc in peers {
        assert_eq!((arc.coverage() * 10.0).round() / 10.0, 0.5);
    }
}

#[tokio::test(threaded_scheduler)]
async fn test_arc_gaps() {
    let conductor = SweetConductor::from_config(Default::default()).await;
    let keystore = conductor.keystore();
    let min_peers = 40;
    let converge = |peers: &mut Vec<DhtArc>| {
        let mut gaps = true;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let bucket = DhtArcBucket::new(*arc, p.clone());
                let density = bucket.density();
                arc.update_length(density);
                // let bucket = DhtArcBucket::new(*arc, p.clone());
                // println!("{}\n{}", bucket, bucket.density().est_gap());
                // println!("{}", bucket.density().est_gap());
            }
            if gaps {
                gaps = check_for_gaps(peers.clone());
            } else {
                let bucket = DhtArcBucket::new(peers[0], peers.clone());
                assert!(!check_for_gaps(peers.clone()), "{}", bucket);
            }
        }
        assert!(!gaps);
    };
    let test = |x: f64, scale, n, k| async move {
        let mut peers = get_peers(
            (min_peers as f64 * x) as usize,
            &[(MAX_HALF_LENGTH as f64 * scale) as u32 + n],
            k,
        )
        .await;
        converge(&mut peers);
    };
    for &peer_factor in [0.1, 0.5, 0.75, 1.0, 2.0, 4.0].iter() {
        for &(scale, n) in [
            (0.0, 20),
            (0.0, 0),
            (1.0, 0),
            (0.75, 0),
            (0.5, 0),
            (0.25, 0),
            (0.125, 0),
        ]
        .iter()
        {
            for i in 0..10 {
                println!("Test: {} peers {}", i, peer_factor * min_peers as f64);
                test(peer_factor, scale, n, keystore.clone()).await;
            }
        }
    }
}

#[tokio::test(threaded_scheduler)]
async fn test_arc_redundancy() {
    let conductor = SweetConductor::from_config(Default::default()).await;
    let keystore = conductor.keystore();
    let min_peers = 40;
    let converge = |peers: &mut Vec<DhtArc>| {
        let mut mature = false;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let bucket = DhtArcBucket::new(*arc, p.clone());
                let density = bucket.density();
                arc.update_length(density);
                // let bucket = DhtArcBucket::new(*arc, p.clone());
                // println!("{}\n{}", bucket, bucket.density().est_gap());
                // println!("{}", bucket.density().est_gap());
            }
            let bucket = DhtArcBucket::new(DhtArc::new(0, MAX_HALF_LENGTH), peers.clone());

            let r = bucket.density().est_total_redundancy();
            if mature {
                assert!(r >= 20);
            } else {
                println!("{}\n{}", bucket, r);
                if r >= 20 {
                    mature = true;
                }
            }
        }
        assert!(mature);
    };
    let test = |x: f64, scale, n, k| async move {
        let mut peers = get_peers(
            (min_peers as f64 * x) as usize,
            &[(MAX_HALF_LENGTH as f64 * scale) as u32 + n],
            k,
        )
        .await;
        converge(&mut peers);
    };
    for &peer_factor in [1.0, 1.1, 1.5, 2.0, 4.0].iter() {
        for &(scale, n) in [
            (0.0, 20),
            (0.0, 0),
            (1.0, 0),
            (0.75, 0),
            (0.5, 0),
            (0.25, 0),
            (0.125, 0),
        ]
        .iter()
        {
            for i in 0..10 {
                println!("Test: {} peers {}", i, peer_factor * min_peers as f64);
                test(peer_factor, scale, n, keystore.clone()).await;
            }
        }
    }
}
