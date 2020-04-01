use crate::conductor::{api::*, interface::interface::*};
//use async_trait::async_trait;
//use tracing::*;

use holochain_serialized_bytes::SerializedBytes;

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ExternalConductorApi
pub fn create_demo_channel_interface<A: ExternalConductorApi>(
    api: A,
) -> (
    futures::channel::mpsc::Sender<(SerializedBytes, ExternalSideResponder)>,
    tokio::task::JoinHandle<()>,
) {
    let (send_sig, _recv_sig) = futures::channel::mpsc::channel(1);
    let (send_req, recv_req) = futures::channel::mpsc::channel(1);

    let (_chan_sig_send, chan_req_recv): (
        ConductorSideSignalSender<SerializedBytes>, // stub impl signals
        ConductorSideRequestReceiver<ConductorRequest, ConductorResponse>,
    ) = create_interface_channel(send_sig, recv_req);

    let join_handle = attach_external_conductor_api(api.clone(), chan_req_recv);

    (send_req, join_handle)
}
