//! Fixturator definitions for kitsune_p2p.

use crate::agent_store::AgentInfo;
use crate::agent_store::AgentInfoSigned;
use crate::agent_store::AgentMetaInfo;
use crate::agent_store::Urls;
use crate::dependencies::url2;
use crate::KitsuneAgent;
use crate::KitsuneBinType;
use crate::KitsuneSignature;
use crate::KitsuneSpace;
use ::fixt::prelude::*;
use url2::url2;

fixturator!(
    Urls;
    curve Empty vec![];
    curve Unpredictable {
        let mut rng = ::fixt::rng();
        let vec_len = rng.gen_range(0, 5);
        let mut ret = vec![];

        for _ in 0..vec_len {
            ret.push(url2!("https://example.com/{}", fixt!(String)));
        }
        ret
    };
    curve Predictable {
        let mut rng = ::fixt::rng();
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
    constructor fn new(ThirtySixBytes);
);

fixturator!(
    KitsuneSpace;
    constructor fn new(ThirtySixBytes);
);

fixturator!(
    KitsuneSignature;
    from SixtyFourBytesVec;
);

fixturator!(
    AgentMetaInfo;
    curve Empty AgentMetaInfo { dht_storage_arc_half_length: u32::MAX / 4 };
    curve Unpredictable AgentMetaInfo { dht_storage_arc_half_length: u32::MAX / 4 };
    curve Predictable AgentMetaInfo { dht_storage_arc_half_length: u32::MAX / 4 };
);

fixturator!(
    AgentInfo;
    curve Empty {
        AgentInfo::new(
            fixt!(KitsuneSpace, Empty),
            fixt!(KitsuneAgent, Empty),
            fixt!(Urls, Empty),
            0,
            0,
        ).with_meta_info(fixt!(AgentMetaInfo, Empty)).unwrap()
    };
    curve Unpredictable {
        AgentInfo::new(
            fixt!(KitsuneSpace, Unpredictable),
            fixt!(KitsuneAgent, Unpredictable),
            fixt!(Urls, Unpredictable),
            0,
            0,
        ).with_meta_info(fixt!(AgentMetaInfo, Unpredictable)).unwrap()
    };
    curve Predictable {
        AgentInfo::new(
            fixt!(KitsuneSpace, Predictable),
            fixt!(KitsuneAgent, Predictable),
            fixt!(Urls, Predictable),
            0,
            0,
        ).with_meta_info(fixt!(AgentMetaInfo, Predictable)).unwrap()
    };
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
