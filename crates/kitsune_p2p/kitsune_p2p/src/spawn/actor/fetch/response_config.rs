use crate::spawn::meta_net::MetaNetCon;
use crate::wire;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::{dht, KOpData, KSpace};

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
