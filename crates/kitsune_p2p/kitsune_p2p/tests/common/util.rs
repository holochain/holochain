use std::sync::Arc;

use ghost_actor::GhostSender;
use kitsune_p2p::actor::{KitsuneP2p, KitsuneP2pSender};
use kitsune_p2p_bin_data::{KitsuneAgent, KitsuneSpace};

pub async fn wait_for_connected(
    sender: GhostSender<KitsuneP2p>,
    to_agent: Arc<KitsuneAgent>,
    space: Arc<KitsuneSpace>,
) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async move {
        loop {
            match sender
                .rpc_single(
                    space.clone(),
                    to_agent.clone(),
                    "connection test".as_bytes().to_vec(),
                    Some(std::time::Duration::from_secs(10).as_millis() as u64),
                )
                .await
            {
                Ok(resp) => {
                    return resp;
                }
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    })
    .await
    .unwrap();
}
