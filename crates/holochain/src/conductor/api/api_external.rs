#![deny(missing_docs)]

use super::error::{ConductorApiError, ConductorApiResult, SerializationError, WireError};
use crate::conductor::{
    interface::error::{InterfaceError, InterfaceResult},
    ConductorHandle,
};
use holochain_serialized_bytes::prelude::*;
use std::path::PathBuf;
use sx_types::{
    cell::CellHandle,
    dna::{Dna, Properties},
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    prelude::*,
};
use tracing::*;

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

/// A trait for the interface that a Conductor exposes to the outside world to use for administering the conductor.
/// This trait has a one mock implementation and one "Real" implementation
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    /// Route the request to be handled
    async fn handle_request(&self, request: AdminRequest) -> AdminResponse {
        let res = self.admin(request).await;

        match res {
            Ok(response) => response,
            Err(ConductorApiError::Io(e)) => {
                AdminResponse::Error(WireError::InvalidDnaPath(format!("{:?}", e)))
            }
            Err(e) => AdminResponse::Error(e.into()),
        }
    }
}
/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AppInterfaceApi: 'static + Send + Sync + Clone {
    /// Invoke a zome function on any cell in this conductor.
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse>;

    // -- provided -- //

    /// Routes the [AppRequest] to the [AppResponse]
    async fn handle_request(&self, request: AppRequest) -> AppResponse {
        let res: ConductorApiResult<AppResponse> = async move {
            match request {
                AppRequest::ZomeInvocationRequest { request } => {
                    Ok(AppResponse::ZomeInvocationResponse {
                        response: Box::new(self.invoke_zome(*request).await?),
                    })
                }
                _ => unimplemented!(),
            }
        }
        .await;

        match res {
            Ok(response) => response,
            Err(e) => AppResponse::Error {
                debug: format!("{:?}", e),
            },
        }
    }
}

/// The admin interface that external connections
/// can use to make requests to the conductor
/// The concrete (non-mock) implementation of the AdminInterfaceApi
#[derive(Clone)]
pub struct RealAdminInterfaceApi {
    /// Mutable access to the Conductor
    conductor_handle: ConductorHandle,

    /// Needed to spawn an App interface
    app_api: RealAppInterfaceApi,
}

impl RealAdminInterfaceApi {
    pub(crate) fn new(conductor_handle: ConductorHandle) -> Self {
        let app_api = RealAppInterfaceApi::new(conductor_handle.clone());
        RealAdminInterfaceApi {
            conductor_handle,
            app_api,
        }
    }

    /// Installs a [Dna] from a file path
    pub(crate) async fn install_dna(
        &self,
        dna_path: PathBuf,
        properties: Option<serde_json::Value>,
    ) -> ConductorApiResult<AdminResponse> {
        trace!(?dna_path);
        let dna = Self::read_parse_dna(dna_path, properties).await?;
        self.add_dna(dna).await?;
        Ok(AdminResponse::DnaInstalled)
    }

    /// Adds the [Dna] to the dna store
    async fn add_dna(&self, dna: Dna) -> ConductorApiResult<()> {
        self.conductor_handle
            .write()
            .await
            .dna_store_mut()
            .add(dna)
            .map_err(|e| e.into())
    }

    /// Reads the [Dna] from disk and parses to [SerializedBytes]
    async fn read_parse_dna(
        dna_path: PathBuf,
        properties: Option<serde_json::Value>,
    ) -> ConductorApiResult<Dna> {
        let dna: UnsafeBytes = tokio::fs::read(dna_path).await?.into();
        let dna = SerializedBytes::from(dna);
        let mut dna: Dna = dna.try_into().map_err(|e| SerializationError::from(e))?;
        if let Some(properties) = properties {
            let properties = Properties::new(properties);
            dna.properties = (properties)
                .try_into()
                .map_err(|e| SerializationError::from(e))?;
        }
        Ok(dna)
    }

    /// Lists all the [Dna]'s in the dna store
    pub(crate) async fn list_dnas(&self) -> ConductorApiResult<AdminResponse> {
        let dna_list = self.conductor_handle.read().await.dna_store().list();
        Ok(AdminResponse::ListDnas(dna_list))
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for RealAdminInterfaceApi {
    async fn admin(&self, request: AdminRequest) -> ConductorApiResult<AdminResponse> {
        use AdminRequest::*;
        match request {
            Start(_cell_handle) => unimplemented!(),
            Stop(_cell_handle) => unimplemented!(),
            InstallDna(dna_path, properties) => self.install_dna(dna_path, properties).await,
            ListDnas => self.list_dnas().await,
        }
    }
}

#[async_trait::async_trait]
impl InterfaceApi for RealAdminInterfaceApi {
    type ApiRequest = AdminRequest;
    type ApiResponse = AdminResponse;

    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        // Don't hold the read across both awaits
        {
            self.conductor_handle
                .read()
                .await
                // Make sure the conductor is not in the process of shutting down
                .check_running()
                .map_err(InterfaceError::RequestHandler)?;
        }
        match request {
            Ok(request) => Ok(AdminInterfaceApi::handle_request(self, request).await),
            Err(e) => Ok(AdminResponse::Error(SerializationError::from(e).into())),
        }
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct RealAppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl RealAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        Self { conductor_handle }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for RealAppInterfaceApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_handle.read().await;
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl InterfaceApi for RealAppInterfaceApi {
    type ApiRequest = AppRequest;
    type ApiResponse = AppResponse;
    async fn handle_request(
        &self,
        request: Result<Self::ApiRequest, SerializedBytesError>,
    ) -> InterfaceResult<Self::ApiResponse> {
        self.conductor_handle
            .read()
            .await
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        match request {
            Ok(request) => Ok(AppInterfaceApi::handle_request(self, request).await),
            Err(e) => Ok(AppResponse::Error {
                debug: e.to_string(),
            }),
        }
    }
}
/// Responses to requests received on an App interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppResponse {
    /// There has been an error in the request
    Error {
        // TODO maybe this could be serialized instead of stringified?
        /// Stringified version of the error
        debug: String,
    },
    /// The response to a zome call
    ZomeInvocationResponse {
        /// The data that was returned by this call
        response: Box<ZomeInvocationResponse>,
    },
}

/// Responses to messages received on an Admin interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminResponse {
    /// This response is unimplemented
    Unimplemented(AdminRequest),
    /// [Dna] has successfully been installed
    DnaInstalled,
    /// A list of all installed [Dna]s
    ListDnas(Vec<Address>),
    /// An error has ocurred in this request
    Error(WireError),
}

/// The set of messages that a conductor understands how to handle over an App interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Asks the conductor to do some crypto
    CryptoRequest {
        /// The request payload
        request: Box<CryptoRequest>,
    },
    /// Call a zome function
    ZomeInvocationRequest {
        /// Information about which zome call you want to make
        request: Box<ZomeInvocation>,
    },
}

/// The set of messages that a conductor understands how to handle over an Admin interface
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum AdminRequest {
    /// Start a cell running
    Start(CellHandle),
    /// Stop a cell running
    Stop(CellHandle),
    /// Install a [Dna] from a path with optional properties
    InstallDna(PathBuf, Option<serde_json::Value>),
    /// List all installed [Dna]s
    ListDnas,
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename = "snake-case", tag = "type", content = "data")]
pub enum TestRequest {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::conductor::RealConductor;
    use anyhow::Result;
    use matches::assert_matches;
    use sx_types::test_utils::{fake_dna, fake_dna_file};
    use uuid::Uuid;

    #[tokio::test]
    async fn install_list_dna() -> Result<()> {
        let conductor = RealConductor::builder().test().await?;
        let admin_api = RealAdminInterfaceApi::new(conductor);
        let uuid = Uuid::new_v4();
        let dna = fake_dna(&uuid.to_string());
        let dna_address = dna.address();
        admin_api.add_dna(dna).await?;
        let dna_list = admin_api.list_dnas().await?;
        let expects = vec![dna_address];
        assert_matches!(dna_list, AdminResponse::ListDnas(a) if a == expects);
        Ok(())
    }

    #[tokio::test]
    async fn dna_read_parses() -> Result<()> {
        let uuid = Uuid::new_v4();
        let mut dna = fake_dna(&uuid.to_string());
        let (dna_path, _tmpdir) = fake_dna_file(dna.clone())?;
        let json = serde_json::json!({
            "test": "example",
            "how_many": 42,
        });
        let properties = Some(json.clone());
        let result = RealAdminInterfaceApi::read_parse_dna(dna_path, properties).await?;
        let properties = Properties::new(json);
        dna.properties = properties.try_into().unwrap();
        assert_eq!(dna, result);
        Ok(())
    }
}
