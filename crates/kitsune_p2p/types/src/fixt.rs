//! Fixturators for Kitsune P2p types

use crate::agent_info::AgentInfoSigned;
use crate::agent_info::UrlList;
use ::fixt::prelude::*;
use kitsune_p2p_bin_data::fixt::KitsuneAgentFixturator;
use kitsune_p2p_bin_data::fixt::KitsuneSignatureFixturator;
use kitsune_p2p_bin_data::fixt::KitsuneSpaceFixturator;
use std::sync::Arc;
use url2::url2;

fixturator!(
    UrlList;
    curve Empty vec![];
    curve Unpredictable {
        let mut rng = ::fixt::rng();
        let vec_len = rng.gen_range(1..3);
        let mut ret = vec![];

        for _ in 0..vec_len {
            let s = fixt!(String).chars().take(10).collect::<String>();
            ret.push(url2!("https://example.com/{}", s).into());
        }
        ret
    };
    curve Predictable {
        let mut rng = ::fixt::rng();
        let vec_len = rng.gen_range(1..3);
        let mut ret = vec![];

        for _ in 0..vec_len {
            let s = fixt!(String, Predictable).chars().take(10).collect::<String>();
            ret.push(url2!("https://example.com/{}", s).into());
        }
        ret
    };
);

fixturator!(
    AgentInfoSigned;
    curve Empty {
        block_on(async move {
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
        block_on(async move {
            AgentInfoSigned::sign(
                Arc::new(fixt!(KitsuneSpace, Unpredictable)),
                Arc::new(fixt!(KitsuneAgent, Unpredictable)),
                u32::MAX / 4,
                fixt!(UrlList, Unpredictable),
                0,
                0,
                |_| async move {
                    Ok(Arc::new(fixt!(KitsuneSignature, Unpredictable)))
                },
            ).await.unwrap()
        })
    };
    curve Predictable {
        block_on(async move {
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

/// make fixturators sync for now
fn block_on<F>(f: F) -> F::Output
where
    F: 'static + std::future::Future + Send,
    F::Output: 'static + Send,
{
    tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(f))
}
