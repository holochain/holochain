use super::error::{ConductorApiError, ConductorApiResult, SerializationError};
use crate::conductor::{
    interface::error::{InterfaceError, InterfaceResult},
    ConductorHandle,
};
use holochain_serialized_bytes::prelude::*;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use sx_types::{
    cell::CellHandle,
    dna::Dna,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[async_trait::async_trait]
pub trait InterfaceApi: 'static + Send + Sync + Clone {
    type ApiRequest: TryFrom<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    type ApiResponse: TryInto<SerializedBytes, Error = SerializedBytesError> + Send + Sync;
    async fn handle_request(&self, request: Self::ApiRequest)
        -> InterfaceResult<Self::ApiResponse>;
}

/// The interface that a Conductor exposes to the outside world.
#[async_trait::async_trait]
pub trait AdminInterfaceApi: 'static + Send + Sync + Clone {
    /// Call an admin function to modify this Conductor's behavior
    async fn admin(&self, method: AdminRequest) -> ConductorApiResult<AdminResponse>;

    // -- provided -- //

    async fn handle_request(&self, request: AdminRequest) -> AdminResponse {
        let res = self.admin(request).await;

        match res {
            Ok(response) => response,
            Err(e) => AdminResponse::Error {
                debug: format!("{:?}", e),
            },
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

#[derive(Clone)]
pub struct StdAdminInterfaceApi {
    conductor_handle: ConductorHandle,
    app_api: StdAppInterfaceApi,
    fake_dna_cache: Arc<RwLock<HashMap<Uuid, Dna>>>,
}

impl StdAdminInterfaceApi {
    pub(crate) fn new(conductor_handle: ConductorHandle) -> Self {
        let app_api = StdAppInterfaceApi::new(conductor_handle.clone());
        let fake_dna_cache = Arc::new(RwLock::new(HashMap::new()));
        StdAdminInterfaceApi {
            conductor_handle,
            app_api,
            fake_dna_cache,
        }
    }

    async fn install_dna(&self, dna_path: PathBuf) -> ConductorApiResult<AdminResponse> {
        let dna: UnsafeBytes = tokio::fs::read(dna_path).await?.into();
        let dna = SerializedBytes::from(dna);
        let dna: Dna = dna.try_into().map_err(SerializationError::from)?;
        {
            let mut fake_dna_cache = self.fake_dna_cache.write().await;
            fake_dna_cache.insert(
                Uuid::parse_str(&dna.uuid).map_err(SerializationError::from)?,
                dna,
            );
        }
        Ok(AdminResponse::DnaInstalled)
    }

    async fn list_dnas(&self) -> ConductorApiResult<AdminResponse> {
        let fake_dna_cache = self.fake_dna_cache.read().await;
        let dna_list = fake_dna_cache.keys().cloned().collect::<Vec<_>>();
        Ok(AdminResponse::ListDnas(dna_list))
    }
}

#[async_trait::async_trait]
impl AdminInterfaceApi for StdAdminInterfaceApi {
    async fn admin(&self, request: AdminRequest) -> ConductorApiResult<AdminResponse> {
        use AdminRequest::*;
        match request {
            Start(_cell_handle) => unimplemented!(),
            Stop(_cell_handle) => unimplemented!(),
            InstallDna(dna_path) => self.install_dna(dna_path).await,
            ListDnas => self.list_dnas().await,
        }
    }
}

#[async_trait::async_trait]
impl InterfaceApi for StdAdminInterfaceApi {
    type ApiRequest = AdminRequest;
    type ApiResponse = AdminResponse;
    async fn handle_request(
        &self,
        request: Self::ApiRequest,
    ) -> InterfaceResult<Self::ApiResponse> {
        self.conductor_handle
            .read()
            .await
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        let r = AdminInterfaceApi::handle_request(self, request).await;
        Ok(r)
    }
}

/// The Conductor lives inside an Arc<RwLock<_>> which is shared with all
/// other Api references
#[derive(Clone)]
pub struct StdAppInterfaceApi {
    conductor_handle: ConductorHandle,
}

impl StdAppInterfaceApi {
    /// Create a new instance from a shared Conductor reference
    pub fn new(conductor_handle: ConductorHandle) -> Self {
        Self { conductor_handle }
    }
}

#[async_trait::async_trait]
impl AppInterfaceApi for StdAppInterfaceApi {
    async fn invoke_zome(
        &self,
        _invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResponse> {
        let _conductor = self.conductor_handle.read().await;
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl InterfaceApi for StdAppInterfaceApi {
    type ApiRequest = AppRequest;
    type ApiResponse = AppResponse;
    async fn handle_request(
        &self,
        request: Self::ApiRequest,
    ) -> InterfaceResult<Self::ApiResponse> {
        self.conductor_handle
            .read()
            .await
            .check_running()
            .map_err(InterfaceError::RequestHandler)?;
        let r = AppInterfaceApi::handle_request(self, request).await;
        Ok(r)
    }
}
/// The set of messages that a conductor understands how to respond
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type")]
pub enum AppResponse {
    Error {
        debug: String,
    },
    ZomeInvocationResponse {
        response: Box<ZomeInvocationResponse>,
    },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type")]
pub enum AdminResponse {
    Unimplemented(AdminRequest),
    DnaInstalled,
    ListDnas(Vec<Uuid>),
    Error { debug: String },
}

/// The set of messages that a conductor understands how to handle
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type")]
pub enum AppRequest {
    CryptoRequest { request: Box<CryptoRequest> },
    TestRequest { request: Box<TestRequest> },
    ZomeInvocationRequest { request: Box<ZomeInvocation> },
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename = "snake-case", tag = "type")]
pub enum AdminRequest {
    Start(CellHandle),
    Stop(CellHandle),
    InstallDna(PathBuf),
    ListDnas,
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum CryptoRequest {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

#[allow(missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TestRequest {
    AddAgent(AddAgentArgs),
}

#[allow(dead_code, missing_docs)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AddAgentArgs {
    id: String,
    name: String,
}
