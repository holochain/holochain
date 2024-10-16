use crate::{KitsuneAgent, KitsuneBinType, KitsuneSignature, KitsuneSpace};
use kitsune_p2p_types::{agent_info::AgentInfoSigned, dht::arq::ArqSize};
use std::sync::Arc;

pub async fn mk_agent_info(u: u8) -> AgentInfoSigned {
    AgentInfoSigned::sign(
        Arc::new(KitsuneSpace::new(vec![0x11; 32])),
        Arc::new(KitsuneAgent::new(vec![u; 32])),
        ArqSize::empty(),
        vec![],
        0,
        0,
        true,
        |_| async move { Ok(Arc::new(KitsuneSignature(vec![0; 64]))) },
    )
    .await
    .unwrap()
}
