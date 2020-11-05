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
        let mut data = Vec::new();
        kitsune_p2p_types::codec::rmp_encode(&mut data, &fixt!(AgentInfo, Empty)).unwrap();
        AgentInfoSigned::try_new(
            fixt!(KitsuneAgent, Empty),
            fixt!(KitsuneSignature, Empty),
            data,
        ).unwrap()
    };
    curve Unpredictable {
        let mut data = Vec::new();
        kitsune_p2p_types::codec::rmp_encode(&mut data, &fixt!(AgentInfo)).unwrap();
        AgentInfoSigned::try_new(
            fixt!(KitsuneAgent),
            fixt!(KitsuneSignature),
            data,
        ).unwrap()
    };
    curve Predictable {
        let mut data = Vec::new();
        kitsune_p2p_types::codec::rmp_encode(&mut data, &fixt!(AgentInfo, Predictable)).unwrap();
        AgentInfoSigned::try_new(
            fixt!(KitsuneAgent, Predictable),
            fixt!(KitsuneSignature, Predictable),
            data,
        ).unwrap()
    };
);
