use crate::conductor::interface::error::InterfaceResult;
use holochain_serialized_bytes::prelude::*;

mod admin_interface;
mod app_interface;
pub use admin_interface::*;
pub use app_interface::*;
use holochain_types::prelude::InstalledAppId;

/// A trait that unifies both the admin and app interfaces
#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    /// An authentication payload to establish a connection.
    /// This is the first message sent on a connection.
    type Auth: TryFrom<SerializedBytes, Error = SerializedBytesError>
        + Send
        + Sync
        + std::fmt::Debug;

    /// Which request is being made
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError>
        + Send
        + Sync
        + std::fmt::Debug;

    /// Which response is sent to the above request
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError>
        + Send
        + Sync
        + std::fmt::Debug;

    /// Authentication a connection request.
    async fn auth(
        &self,
        auth: Self::Auth,
    ) -> InterfaceResult<InstalledAppId>;

    /// Handle a request on this API
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse>;
}
