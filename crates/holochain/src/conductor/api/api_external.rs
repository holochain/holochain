use crate::conductor::interface::error::InterfaceResult;
use holochain_serialized_bytes::prelude::*;

mod admin_interface;
mod app_interface;
pub use admin_interface::*;
pub use app_interface::*;

/// A trait that unifies both the admin and app interfaces
#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    /// Which request is being made
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    /// Which response is sent to the above request
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    /// Handle a request on this API
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse>;
}

#[macro_export]
macro_rules! impl_handler {
    ($enum: ident . $res: ident (Box) <= $fut: expr) => {
        match $fut.await.map_err(|e| ExternalApiWireError::internal(e))? {
            $enum::$res(v) => Ok(*v),
            $enum::Error(err) => Err(err),
            r => Err(ExternalApiWireError::internal(format!(
                "Invalid return value, expected a {}::{} but got: {:?}",
                stringify!($enum),
                stringify!($res),
                r
            ))),
        }
    };
    ($enum: ident . $res: ident (_) <= $fut: expr) => {
        match $fut.await.map_err(|e| ExternalApiWireError::internal(e))? {
            $enum::$res(v) => Ok(v),
            $enum::Error(err) => Err(err),
            r => Err(ExternalApiWireError::internal(format!(
                "Invalid return value, expected a {}::{} but got: {:?}",
                stringify!($enum),
                stringify!($res),
                r
            ))),
        }
    };
    ($enum: ident . $res: ident <= $fut: expr) => {
        match $fut.await.map_err(|e| ExternalApiWireError::internal(e))? {
            $enum::$res => Ok(()),
            $enum::Error(err) => Err(err),
            r => Err(ExternalApiWireError::internal(format!(
                "Invalid return value, expected a {}::{} but got: {:?}",
                stringify!($enum),
                stringify!($res),
                r
            ))),
        }
    };
}
