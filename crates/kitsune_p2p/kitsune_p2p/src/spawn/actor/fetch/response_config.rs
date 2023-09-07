use crate::spawn::meta_net::MetaNetCon;
use crate::wire;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::{dht, KOpData, KSpace};

#[derive(Clone)]
pub struct FetchResponseConfig(KitsuneP2pTuningParams);

impl FetchResponseConfig {
    pub fn new(params: KitsuneP2pTuningParams) -> Self {
        FetchResponseConfig(params)
    }
}

impl kitsune_p2p_fetch::FetchResponseConfig for FetchResponseConfig {
    type User = (
        MetaNetCon,
        String,
        Option<(dht::prelude::RegionCoords, bool)>,
    );

    fn respond(
        &self,
        space: KSpace,
        user: Self::User,
        completion_guard: kitsune_p2p_fetch::FetchResponseGuard,
        op: KOpData,
    ) {
        let timeout = self.0.implicit_timeout();
        tokio::task::spawn(async move {
            let _completion_guard = completion_guard;

            // MAYBE: open a new connection if the con was closed??
            let (con, _url, region) = user;

            let item = wire::PushOpItem {
                op_data: op,
                region,
            };
            tracing::debug!("push_op_data: {:?}", item);
            let payload = wire::Wire::push_op_data(vec![(space, vec![item])]);

            if let Err(err) = con.notify(&payload, timeout).await {
                tracing::warn!(?err, "error responding to op fetch");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::spawn::actor::fetch::FetchResponseConfig as RealFetchResponseConfig;
    use crate::spawn::meta_net::{MetaNetCon, MetaNetConTest};
    use kitsune_p2p_fetch::test_utils::test_space;
    use kitsune_p2p_fetch::{FetchResponseConfig, FetchResponseGuard};
    use kitsune_p2p_types::bin_types::KitsuneOpData;
    use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
    use kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::parking_lot::lock_api::RwLock;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn responds_by_doing_notify() {
        let config = RealFetchResponseConfig::new(Arc::new(KitsuneP2pTuningParams::default()));

        let (s, _r) = tokio::sync::oneshot::channel();

        let connection_state = Arc::new(RwLock::new(MetaNetConTest::default()));
        config.respond(
            test_space(1),
            (
                MetaNetCon::Test {
                    state: connection_state.clone(),
                },
                "".to_string(),
                None,
            ),
            FetchResponseGuard::new(s),
            Arc::new(KitsuneOpData(vec![1, 2, 3])),
        );

        tokio::time::timeout(Duration::from_millis(100), {
            let connection_state = connection_state.clone();
            async move {
                while connection_state.read().notify_call_count < 1 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(1, connection_state.read().notify_call_count);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handles_error_during_notify() {
        let config = RealFetchResponseConfig::new(Arc::new(KitsuneP2pTuningParams::default()));

        let (s, _r) = tokio::sync::oneshot::channel();

        let connection_state = Arc::new(RwLock::new(MetaNetConTest::default()));
        connection_state.write().notify_succeed = false;

        config.respond(
            test_space(1),
            (
                MetaNetCon::Test {
                    state: connection_state.clone(),
                },
                "".to_string(),
                None,
            ),
            FetchResponseGuard::new(s),
            Arc::new(KitsuneOpData(vec![1, 2, 3])),
        );

        tokio::time::timeout(Duration::from_millis(100), {
            let connection_state = connection_state.clone();
            async move {
                while connection_state.read().notify_call_count < 1 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(1, connection_state.read().notify_call_count);
    }
}
