use crate::{actor, actor::*, event::*};

use crate::holo_hash_core::HoloHashCoreHash;
use futures::future::FutureExt;

ghost_actor::ghost_chan! {
    Visibility(pub(crate)),
    Name(Internal),
    Error(crate::HolochainP2pError),
    Api {
        Ping(
            "temp because ghost_chan doesn't allow empty Api",
            (),
            (),
        ),
    }
}

pub(crate) struct HolochainP2pActor {
    #[allow(dead_code)]
    internal_sender: HolochainP2pInternalSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
    kitsune_p2p: kitsune_p2p::actor::KitsuneP2pSender,
}

impl HolochainP2pActor {
    pub async fn new(
        internal_sender: HolochainP2pInternalSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
    ) -> HolochainP2pResult<Self> {
        let (kitsune_p2p, _kitsune_p2p_events) = kitsune_p2p::spawn_kitsune_p2p().await?;
        // TODO - task for handling p2p_events
        Ok(Self {
            internal_sender,
            evt_sender,
            kitsune_p2p,
        })
    }
}

impl HolochainP2pHandler<(), Internal> for HolochainP2pActor {
    fn handle_join(&mut self, input: actor::Join) -> HolochainP2pHandlerResult<()> {
        let actor::Join {
            dna_hash,
            agent_pub_key,
        } = input;
        let space: kitsune_p2p::KitsuneSpace = dna_hash.into_inner().into();
        let agent: kitsune_p2p::KitsuneAgent = agent_pub_key.into_inner().into();

        let mut kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            Ok(kitsune_p2p
                .join(kitsune_p2p::actor::Join { space, agent })
                .await?)
        }
        .boxed()
        .into())
    }

    fn handle_leave(&mut self, _input: actor::Leave) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_call_remote(&mut self, _input: actor::CallRemote) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_publish(&mut self, _input: actor::Publish) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_validation_package(
        &mut self,
        _input: actor::GetValidationPackage,
    ) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get(&mut self, _input: actor::Get) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_links(&mut self, _input: actor::GetLinks) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }
}
