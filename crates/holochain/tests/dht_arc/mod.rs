use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::dht_arc::PeerStratAlpha;
use holochain_p2p::dht_arc::DEFAULT_MIN_PEERS;
use holochain_p2p::dht_arc::DEFAULT_MIN_REDUNDANCY;
use holochain_p2p::dht_arc::MAX_HALF_LENGTH;
use kitsune_p2p::dht_arc::DhtArc;
use kitsune_p2p::*;
use kitsune_p2p_types::dht_arc::check_redundancy;
use kitsune_p2p_types::dht_arc::gaps::check_for_gaps;

async fn get_peers(num: usize, half_lens: &[u32], keystore: MetaLairClient) -> Vec<DhtArc> {
    let mut half_lens = half_lens.iter().cycle();
    let mut out = Vec::with_capacity(num);

    let agents = SweetAgents::get(keystore, num).await;
    for agent in agents {
        let agent = holochain_p2p::agent_holo_to_kit(agent);
        let arc = DhtArc::from_start_and_half_len(agent.get_loc(), *half_lens.next().unwrap());
        out.push(arc);
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Not using alpha strategy anymore, todo: change to new strategy"]
// This test shows that we can handle maintaining our [`MINIMUM_REDUNDANCY`]
// through 1000 trials. If this test ever fails it's not flakey. Instead that means
// we can't actually maintain the [`MIN_REDUNDANCY`] and will need raise the [`REDUNDANCY_TARGET`].
// Please @freesig if you see this fail.
async fn test_arc_redundancy() {
    let conductor = SweetConductor::from_config(Default::default()).await;
    let keystore = conductor.keystore();
    fn converge(peers: &mut Vec<DhtArc>) {
        let mut mature = false;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let view = PeerStratAlpha::default().view(*arc, p.as_slice()).into();
                arc.update_length(&view);
            }

            assert!(!check_for_gaps(peers.clone()));
            let r = check_redundancy(peers.clone());
            if mature {
                assert!(r >= DEFAULT_MIN_REDUNDANCY);
            } else {
                if r >= DEFAULT_MIN_REDUNDANCY {
                    mature = true;
                }
            }
        }
        assert!(mature);
    }
    let mut jhs = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let jh = tokio::spawn({
            let keystore = keystore.clone();
            async move {
                let mut peers = get_peers(
                    (DEFAULT_MIN_PEERS as f64 * 1.1) as usize,
                    &[(MAX_HALF_LENGTH as f64 * 0.2) as u32],
                    keystore,
                )
                .await;
                converge(&mut peers);
            }
        });
        jhs.push(jh);
    }
    for jh in jhs {
        jh.await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Slow test that must check lots of combinations of values"]
async fn test_arc_redundancy_all() {
    let conductor = SweetConductor::from_config(Default::default()).await;
    let keystore = conductor.keystore();
    let converge = |peers: &mut Vec<DhtArc>| {
        let mut mature = false;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.clone();
                let arc = peers.get_mut(i).unwrap();
                let view = PeerStratAlpha::default().view(*arc, p.as_slice()).into();
                arc.update_length(&view);
            }

            let r = check_redundancy(peers.clone());
            if mature {
                assert!(r >= DEFAULT_MIN_REDUNDANCY);
            } else {
                if r >= DEFAULT_MIN_REDUNDANCY {
                    mature = true;
                }
            }
        }
        assert!(mature);
    };
    let test = |scale_peers: f64, coverage, additional_coverage, keystore| async move {
        let mut peers = get_peers(
            (DEFAULT_MIN_PEERS as f64 * scale_peers) as usize,
            &[(MAX_HALF_LENGTH as f64 * coverage) as u32 + additional_coverage],
            keystore,
        )
        .await;
        converge(&mut peers);
    };
    for &scale_peers in [1.0, 1.1, 1.5, 2.0].iter() {
        for &(coverage, additional_coverage) in [
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
            for i in 0..100 {
                println!(
                    "Test: {} scale_peers {} peers {} scale {} additional_coverage {}",
                    i,
                    scale_peers,
                    scale_peers * DEFAULT_MIN_PEERS as f64,
                    coverage,
                    additional_coverage
                );
                test(scale_peers, coverage, additional_coverage, keystore.clone()).await;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
// Can survive 50% of the nodes changing per update without
// dropping below [`MIN_REDUNDANCY`]
async fn test_join_leave() {
    let conductor = SweetConductor::from_config(Default::default()).await;
    let keystore = conductor.keystore();

    let num_peers = DEFAULT_MIN_PEERS;

    let coverages = vec![MAX_HALF_LENGTH];
    let converge = |peers: &mut Vec<DhtArc>| {
        for i in 0..peers.len() {
            let p = peers.clone();
            let arc = peers.get_mut(i).unwrap();
            let view = PeerStratAlpha::default()
                .view(arc.clone(), p.as_slice())
                .into();
            arc.update_length(&view);
        }
    };
    let mut peers = get_peers(num_peers, &coverages, keystore.clone()).await;
    let delta_peers = num_peers / 2;
    let mut mature = false;
    for _ in 0..40 {
        let new_peers = get_peers(delta_peers, &coverages, keystore.clone()).await;
        for (o, n) in peers[..delta_peers].iter_mut().zip(new_peers.into_iter()) {
            *o = n;
        }
        converge(&mut peers);
        let r = check_redundancy(peers.clone());

        if mature {
            assert!(r >= DEFAULT_MIN_REDUNDANCY);
        } else {
            if r >= DEFAULT_MIN_REDUNDANCY {
                mature = true;
            }
        }
    }
    assert!(mature);
}
