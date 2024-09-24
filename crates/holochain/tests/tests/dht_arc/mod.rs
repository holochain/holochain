use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::dht::prelude::Topology;
use holochain_p2p::dht::PeerStrat;
use holochain_p2p::dht_arc::DEFAULT_MIN_PEERS;
use holochain_p2p::dht_arc::DEFAULT_MIN_REDUNDANCY;
use holochain_p2p::dht_arc::MAX_HALF_LENGTH;
use kitsune_p2p::dht::spacetime::SpaceDimension;
use kitsune_p2p::dht::Arq;
use kitsune_p2p::dht::ArqStrat;
use kitsune_p2p::*;
use kitsune_p2p_types::dht_arc::check_redundancy;

async fn get_peers(num: usize, half_lens: &[u32], keystore: MetaLairClient) -> Vec<Arq> {
    let mut half_lens = half_lens.iter().cycle();
    let mut out = Vec::with_capacity(num);

    let agents = SweetAgents::get(keystore, num).await;
    for agent in agents {
        let agent = holochain_p2p::agent_holo_to_kit(agent);
        let arq = Arq::from_start_and_half_len_approximate(
            SpaceDimension::standard(),
            &ArqStrat::default(),
            agent.get_loc(),
            *half_lens.next().unwrap(),
        );
        out.push(arq);
    }
    out
}

// TODO This really looks like it shouldn't be disabled!
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Not using alpha strategy anymore, todo: change to new strategy"]
// This test shows that we can handle maintaining our [`MINIMUM_REDUNDANCY`]
// through 1000 trials. If this test ever fails it's not flakey. Instead that means
// we can't actually maintain the [`MIN_REDUNDANCY`] and will need raise the [`REDUNDANCY_TARGET`].
// Please @freesig if you see this fail.
async fn test_arc_redundancy() {
    let conductor = SweetConductor::local_rendezvous().await;
    let keystore = conductor.keystore();
    fn converge(peers: &mut [Arq]) {
        let mut mature = false;
        for _ in 0..40 {
            for i in 0..peers.len() {
                let p = peers.to_owned();
                let arc = peers.get_mut(i).unwrap();
                let view = PeerStrat::default().view(Topology::standard_epoch_full(), p.as_slice());
                view.update_arq(arc);
            }

            let r = check_redundancy(peers.iter().map(|a| a.to_dht_arc_std()).collect());
            if mature {
                assert!(r >= DEFAULT_MIN_REDUNDANCY);
            } else if r >= DEFAULT_MIN_REDUNDANCY {
                mature = true;
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
// Can survive 50% of the nodes changing per update without
// dropping below [`MIN_REDUNDANCY`]
async fn test_join_leave() {
    let conductor = SweetConductor::local_rendezvous().await;
    let keystore = conductor.keystore();
    let topo = Topology::standard_epoch_full();

    let num_peers = DEFAULT_MIN_PEERS;

    let coverages = vec![MAX_HALF_LENGTH];
    let converge = |peers: &mut Vec<Arq>| {
        for i in 0..peers.len() {
            let p = peers.clone();
            let arq = peers.get_mut(i).unwrap();
            let view = PeerStrat::default().view(topo.clone(), p.as_slice());
            view.update_arq(arq);
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
        let r = check_redundancy(peers.iter().map(|a| a.to_dht_arc(&topo)).collect());

        if mature {
            assert!(r >= DEFAULT_MIN_REDUNDANCY);
        } else if r >= DEFAULT_MIN_REDUNDANCY {
            mature = true;
        }
    }
    assert!(mature);
}
