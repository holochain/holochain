//! Fixturator definitions for kitsune_p2p.

use crate::agent_store::AgentInfoSigned;
use crate::agent_store::UrlList;
use crate::dependencies::url2;
use crate::KitsuneAgent;
use crate::KitsuneBinType;
use crate::KitsuneSignature;
use crate::KitsuneSpace;
use ::fixt::prelude::*;
use std::sync::Arc;
use url2::url2;

fixturator!(
    UrlList;
    curve Empty vec![];
    curve Unpredictable {
        let mut rng = ::fixt::rng();
        let vec_len = rng.gen_range(0, 5);
        let mut ret = vec![];

        for _ in 0..vec_len {
            ret.push(url2!("https://example.com/{}", fixt!(String)).into());
        }
        ret
    };
    curve Predictable {
        let mut rng = ::fixt::rng();
        let vec_len = rng.gen_range(0, 5);
        let mut ret = vec![];

        for _ in 0..vec_len {
            ret.push(url2!("https://example.com/{}", fixt!(String, Predictable)).into());
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
    AgentInfoSigned;
    curve Empty {
        tokio::runtime::Handle::current().block_on(async move {
            AgentInfoSigned::sign(
                Arc::new(fixt!(KitsuneSpace, Empty)),
                Arc::new(fixt!(KitsuneAgent, Empty)),
                u32::MAX / 4,
                fixt!(UrlList, Empty),
                0,
                0,
                |_| async move {
                    Ok(Arc::new(fixt!(KitsuneSignature, Empty)))
                },
            ).await.unwrap()
        })
    };
    curve Unpredictable {
        tokio::runtime::Handle::current().block_on(async move {
            AgentInfoSigned::sign(
                Arc::new(fixt!(KitsuneSpace, Unpredictable)),
                Arc::new(fixt!(KitsuneAgent, Unpredictable)),
                u32::MAX / 4,
                fixt!(UrlList, Empty),
                0,
                0,
                |_| async move {
                    Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable)))
                },
            ).await.unwrap()
        })
    };
    curve Predictable {
        tokio::runtime::Handle::current().block_on(async move {
            AgentInfoSigned::sign(
                Arc::new(fixt!(KitsuneSpace, Predictable)),
                Arc::new(fixt!(KitsuneAgent, Predictable)),
                u32::MAX / 4,
                fixt!(UrlList, Empty),
                0,
                0,
                |_| async move {
                    Ok(Arc::new(fixt!(KitsuneSignature, Predictable)))
                },
            ).await.unwrap()
        })
    };
);
