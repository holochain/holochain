use crate::actor::*;
use crate::event::*;

mod actor;
use actor::*;
use holo_hash::DnaHash;

/// Spawn a new HolochainP2p actor.
/// Conductor will call this on initialization.
pub async fn spawn_holochain_p2p(
    config: kitsune_p2p::dependencies::kitsune_p2p_types::config::KitsuneP2pConfig,
    tls_config: kitsune_p2p::dependencies::kitsune_p2p_types::tls::TlsConfig,
    host: kitsune_p2p::HostApi,
    compat: NetworkCompatParams,
) -> HolochainP2pResult<(
    ghost_actor::GhostSender<HolochainP2p>,
    HolochainP2pEventReceiver,
)> {
    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let sender = channel_factory.create_channel::<HolochainP2p>().await?;

    tokio::task::spawn(builder.spawn(
        HolochainP2pActor::new(config, tls_config, channel_factory, evt_send, host, compat).await?,
    ));

    Ok((sender, evt_recv))
}

/// Some parameters used as part of a protocol compability check during tx5 preflight
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NetworkCompatParams {
    /// The UUID of the installed DPKI service.
    /// If the service is backed by a Dna, this is the core 32 bytes of the DnaHash.
    pub dpki_uuid: Option<[u8; 32]>,
}
