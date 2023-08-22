use kitsune_p2p_gossip::{
    codec::GossipMsg,
    mux::GossipMux,
    round::{self, AxInitiate, GossipRound, GossipRoundParams},
    PeerId,
};

use proptest::{collection, prelude::*, sample::SizeRange};
use test_strategy::proptest;

fn gossip_msgs(len: impl Into<SizeRange>) -> BoxedStrategy<Vec<round::Ax>> {
    collection::vec(any::<round::Ax>(), len).boxed()
}

fn proptest_runner() -> proptest::test_runner::TestRunner {
    use proptest::test_runner::{Config, TestRunner};
    TestRunner::new(Config::default())
}

fn proptest_run<S: Strategy>(strategy: S, test: impl Fn(S::Value)) {
    proptest_runner()
        .run(&strategy, move |v| Ok(test(v)))
        .unwrap();
}

#[test]
fn gossip_fuzzy() {
    proptest_run(
        (gossip_msgs(3..12), any::<AxInitiate>()),
        |(msgs, initiate)| {
            let mut mux = GossipMux::default();
            let id = PeerId::default();
            mux.receive(id.clone(), initiate.into());
            for msg in msgs {
                mux.receive(id.clone(), msg.into());
            }
        },
    )
}

// NOTE: this test is identical to the above, just using the attr macro syntax.
// The other test is easier for rust-analyzer to work with.
#[proptest]
fn gossip_fuzz(#[strategy(gossip_msgs(3..12))] msgs: Vec<round::Ax>, initiate: AxInitiate) {
    let mut mux = GossipMux::default();
    let id = PeerId::default();
    mux.receive(id.clone(), initiate.into());
    for msg in msgs {
        mux.receive(id.clone(), msg.into());
    }
}
