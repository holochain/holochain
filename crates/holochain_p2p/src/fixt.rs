//! Fixturator definitions for kitsune_p2p.

use fixt::prelude::*;
use kitsune_p2p::{
    agent_store::{AgentInfo, AgentInfoSigned, Urls},
    dependencies::url2,
    KitsuneAgent, KitsuneSignature, KitsuneSpace,
};
use url2::url2;

fixturator!(
    Urls;
    curve Empty vec![];
    curve Unpredictable {
        let mut rng = fixt::rng();
        let vec_len = rng.gen_range(0, 5);
        let mut ret = vec![];

        for _ in 0..vec_len {
            ret.push(url2!("https://example.com/{}", fixt!(String)));
        }
        ret
    };
    curve Predictable {
        let mut rng = fixt::rng();
        let vec_len = rng.gen_range(0, 5);
        let mut ret = vec![];

        for _ in 0..vec_len {
            ret.push(url2!("https://example.com/{}", fixt!(String, Predictable)));
        }
        ret
    };
);

fixturator!(
    KitsuneAgent;
    from ThirtySixBytes;
);

fixturator!(
    KitsuneSpace;
    from ThirtySixBytes;
);

fixturator!(
    KitsuneSignature;
    from ThirtySixBytes;
);

fixturator!(
    AgentInfo;
    constructor fn new(KitsuneSpace, KitsuneAgent, Urls, U64);
);

fixturator!(
    AgentInfoSigned;
    curve Empty {
        AgentInfoSigned::try_new(fixt!(KitsuneSignature, Empty), fixt!(AgentInfo, Empty)).unwrap()
    };
    curve Unpredictable {
        AgentInfoSigned::try_new(fixt!(KitsuneSignature), fixt!(AgentInfo)).unwrap()
    };
    curve Predictable {
        AgentInfoSigned::try_new(fixt!(KitsuneSignature, Predictable), fixt!(AgentInfo, Predictable)).unwrap()
    };
);
