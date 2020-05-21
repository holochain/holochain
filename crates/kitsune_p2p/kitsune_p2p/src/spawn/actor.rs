use crate::{actor, actor::*, event::*};

use futures::future::FutureExt;

ghost_actor::ghost_chan! {
    Visibility(pub(crate)),
    Name(Internal),
    Error(crate::KitsuneP2pError),
    Api {
        Ping(
            "temp because ghost_chan doesn't allow empty Api",
            (),
            (),
        ),
    }
}

pub(crate) struct KitsuneP2pActor {
    #[allow(dead_code)]
    internal_sender: KitsuneP2pInternalSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
}

impl KitsuneP2pActor {
    pub fn new(
        internal_sender: KitsuneP2pInternalSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> KitsuneP2pResult<Self> {
        Ok(Self {
            internal_sender,
            evt_sender,
        })
    }
}

impl KitsuneP2pHandler<(), Internal> for KitsuneP2pActor {
    fn handle_join(&mut self, _input: actor::Join) -> KitsuneP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_leave(&mut self, _input: actor::Leave) -> KitsuneP2pHandlerResult<()> {
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_request(&mut self, _input: actor::Request) -> KitsuneP2pHandlerResult<Vec<u8>> {
        Ok(async move { Ok(vec![]) }.boxed().into())
    }

    fn handle_broadcast(&mut self, _input: actor::Broadcast) -> KitsuneP2pHandlerResult<u32> {
        Ok(async move { Ok(0) }.boxed().into())
    }

    fn handle_multi_request(
        &mut self,
        _input: actor::MultiRequest,
    ) -> KitsuneP2pHandlerResult<Vec<actor::MultiRequestResponse>> {
        Ok(async move { Ok(vec![]) }.boxed().into())
    }
}
