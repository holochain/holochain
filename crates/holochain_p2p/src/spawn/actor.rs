use crate::{actor, actor::*, event::*};

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
}

impl HolochainP2pActor {
    pub fn new(
        internal_sender: HolochainP2pInternalSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
    ) -> HolochainP2pResult<Self> {
        Ok(Self {
            internal_sender,
            evt_sender,
        })
    }
}

impl HolochainP2pHandler<(), Internal> for HolochainP2pActor {
    fn handle_join(&mut self, _input: actor::Join) -> HolochainP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
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
