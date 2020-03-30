use crate::conductor::api::ExternalConductorApi;
use async_trait::async_trait;

use holochain_serialized_bytes::SerializedBytes;
use std::convert::TryInto;

pub struct ExternIfaceSignalSender<S: TryInto<SerializedBytes>> {
    sender: tokio::sync::mpsc::Sender<SerializedBytes>,
    phantom: std::marker::PhantomData<S>,
}

impl<S: TryInto<SerializedBytes>> ExternIfaceSignalSender<S> {
    pub async fn send(&mut self, data: S) -> Result<(), ()> {
        self.sender.send(data.try_into().map_err(|_| ())?).await.map_err(|_|())
    }

    // -- private -- //

    fn priv_new(sender: tokio::sync::mpsc::Sender<SerializedBytes>) -> Self {
        Self {
            sender,
            phantom: std::marker::PhantomData,
        }
    }
}

// TODO - the request/response part from the remote(websocket)

pub fn create_interface<S: TryInto<SerializedBytes>>(channel_size: usize) -> ExternIfaceSignalSender<S> {
    let (send_signal, _recv_signal) = tokio::sync::mpsc::channel(channel_size);

    ExternIfaceSignalSender::priv_new(send_signal)
}

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ExternalConductorApi);
}

/*
#[async_trait]
pub trait InterfaceConductorSide: 'static + Send {
}

#[async_trait]
pub trait InterfaceExternalSide: 'static + Send {
}

pub type DynInterfaceExternalSide = Box<dyn InterfaceExternalSide + 'static + Send>;

pub struct InterfaceJoint<C: InterfaceConductorSide> {
    conductor_side: C,
    external_sides: Vec<DynInterfaceExternalSide>,
}

impl<C: InterfaceConductorSide> InterfaceJoint<C> {
    pub fn new(conductor_side: C) -> Self {
        Self {
            conductor_side,
        }
    }
}
*/

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn interface_sanity_test() {
        println!("yo");
    }
}
