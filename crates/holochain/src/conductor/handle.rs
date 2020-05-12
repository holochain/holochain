#![deny(missing_docs)]

//! Defines [ConductorHandle], a lightweight cloneable reference to a Conductor
//! with a limited public interface.
//!
//! A ConductorHandle can be produced via [Conductor::into_handle]
//!
//! ```rust, no_run
//! # async fn async_main () {
//! # use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
//! use holochain_2020::conductor::{Conductor, ConductorBuilder, ConductorHandle};
//! # let env = test_conductor_env();
//! #   let TestEnvironment {
//! #       env: wasm_env,
//! #      tmpdir: _tmpdir,
//! # } = test_wasm_env();
//! let conductor: Conductor = ConductorBuilder::new().test(env, wasm_env).await.unwrap();
//!
//! // Do direct manipulation of the Conductor here
//!
//! // move the Conductor into a ConductorHandle,
//! // making the original Conductor inaccessible
//! let handle: ConductorHandle = conductor.run().await.unwrap();
//!
//! // handles are cloneable
//! let handle2 = handle.clone();
//!
//! assert_eq!(handle.list_dnas().await, Ok(vec![]));
//! handle.shutdown().await;
//!
//! // handle2 will only get errors from now on, since the other handle
//! // shut down the conductor.
//! assert!(handle2.list_dnas().await.is_err());
//!
//! # }
//! ```
//!
//! The purpose of this handle is twofold:
//!
//! First, it specifies how to synchronize
//! read/write access to a single Conductor across multiple references. The various
//! Conductor APIs - [CellConductorApi], [AdminInterfaceApi], and [AppInterfaceApi],
//! use a ConductorHandle as their sole method of interaction with a Conductor.
//!
//! Secondly, it hides the concrete type of the Conductor behind a dyn Trait.
//! The Conductor is a central point of configuration, and has several
//! type parameters, used to modify functionality including specifying mock
//! types for testing. If we did not have a way of hiding this type genericity,
//! code which interacted with the Conductor would also have to be highly generic.

use super::{
    api::error::ConductorApiResult, config::AdminInterfaceConfig, dna_store::DnaStore,
    error::ConductorResult, manager::TaskManagerRunHandle, Cell, Conductor,
};
use crate::core::workflow::ZomeInvocationResult;
use derive_more::From;
use holochain_types::{
    autonomic::AutonomicCue, cell::CellId, dna::DnaFile, nucleus::ZomeInvocation, prelude::*,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::*;

#[cfg(test)]
use holochain_state::env::EnvironmentWrite;

/// A handle to the Conductor that can easily be passed around and cheaply cloned
pub type ConductorHandle = Arc<dyn ConductorHandleT>;

/// Base trait for ConductorHandle
#[async_trait::async_trait]
pub trait ConductorHandleT: Send + Sync {
    /// Returns error if conductor is shutting down
    async fn check_running(&self) -> ConductorResult<()>;

    /// Add a collection of Admin interfaces and spawn the necessary tasks.
    ///
    /// This requires a concrete ConductorHandle to be passed into the
    /// interface tasks. This is a bit weird to do, but it was the only way
    /// around having a circular reference in the types.
    ///
    /// Never use a ConductorHandle for different Conductor here!
    async fn add_admin_interfaces_via_handle(
        &self,
        handle: ConductorHandle,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()>;

    /// Install a [Dna] in this Conductor
    async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()>;

    /// Get the list of hashes of installed Dnas in this Conductor
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;

    /// Get a [Dna] from the [DnaStore]
    async fn get_dna(&self, hash: DnaHash) -> Option<DnaFile>;

    /// Invoke a zome function on a Cell
    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult>;

    /// Cue the autonomic system to perform some action early (experimental)
    async fn autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()>;

    /// Get a Websocket port which will
    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

    /// Return the JoinHandle for all managed tasks, which when resolved will
    /// signal that the Conductor has completely shut down
    async fn get_wait_handle(&self) -> Option<TaskManagerRunHandle>;

    /// Send a signal to all managed tasks asking them to end ASAP.
    async fn shutdown(&self);

    /// Request access to this conductor's keystore
    fn keystore(&self) -> &KeystoreSender;

    /// Create the cells from the database
    async fn create_cells(
        &self,
        cell_ids: Vec<CellId>,
        handle: ConductorHandle,
    ) -> ConductorResult<()>;

    // HACK: remove when B-01593 lands
    #[cfg(test)]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite>;
}

/// The current "production" implementation of a ConductorHandle.
/// The implementation specifies how read/write access to the Conductor
/// should be synchronized across multiple concurrent Handles.
///
/// Synchronization is currently achieved via a simple RwLock, but
/// this could be swapped out with, e.g. a channel Sender/Receiver pair
/// using an actor model.
#[derive(From)]
pub struct ConductorHandleImpl<DS: DnaStore + 'static>(RwLock<Conductor<DS>>, KeystoreSender);

#[async_trait::async_trait]
impl<DS: DnaStore + 'static> ConductorHandleT for ConductorHandleImpl<DS> {
    /// Check that shutdown has not been called
    async fn check_running(&self) -> ConductorResult<()> {
        self.0.read().await.check_running()
    }

    async fn add_admin_interfaces_via_handle(
        &self,
        handle: ConductorHandle,
        configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        let mut lock = self.0.write().await;
        lock.add_admin_interfaces_via_handle(handle, configs).await
    }

    async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()> {
        {
            self.0.write().await.put_wasm(dna.clone()).await?;
        }
        Ok(self.0.write().await.dna_store_mut().add(dna)?)
    }

    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        Ok(self.0.read().await.dna_store().list())
    }

    async fn get_dna(&self, hash: DnaHash) -> Option<DnaFile> {
        self.0.read().await.dna_store().get(hash)
    }

    async fn invoke_zome(
        &self,
        invocation: ZomeInvocation,
    ) -> ConductorApiResult<ZomeInvocationResult> {
        // FIXME: D-01058: We are holding this read lock for
        // the entire call to invoke_zome and blocking
        // any writes to the conductor
        let lock = self.0.read().await;
        debug!(cell_id = ?invocation.cell_id);
        let cell: &Cell = lock.cell_by_id(&invocation.cell_id)?;
        cell.invoke_zome(invocation).await.map_err(Into::into)
    }

    async fn autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()> {
        let lock = self.0.write().await;
        let cell = lock.cell_by_id(cell_id)?;
        let _ = cell.handle_autonomic_process(cue.into()).await;
        Ok(())
    }

    async fn get_wait_handle(&self) -> Option<TaskManagerRunHandle> {
        self.0.write().await.get_wait_handle()
    }

    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        self.0.read().await.get_arbitrary_admin_websocket_port()
    }

    async fn shutdown(&self) {
        self.0.write().await.shutdown()
    }

    fn keystore(&self) -> &KeystoreSender {
        &self.1
    }

    async fn create_cells(
        &self,
        cell_ids: Vec<CellId>,
        handle: ConductorHandle,
    ) -> ConductorResult<()> {
        let mut lock = self.0.write().await;
        lock.create_cells(cell_ids, handle).await?;
        Ok(())
    }

    #[cfg(test)]
    async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite> {
        let lock = self.0.read().await;
        let cell = lock.cell_by_id(cell_id)?;
        Ok(cell.state_env())
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use mockall::mock;

    // Unfortunate workaround to get mockall to work with async_trait, due to the complexity of each.
    // The mock! expansion here creates mocks on a non-async version of the API, and then the actual trait is implemented
    // by delegating each async trait method to its sync counterpart
    // See https://github.com/asomers/mockall/issues/75
    mock! {

        pub ConductorHandle {
            fn sync_check_running(&self) -> ConductorResult<()>;

            fn sync_add_admin_interfaces_via_handle(
                &self,
                handle: ConductorHandle,
                configs: Vec<AdminInterfaceConfig>,
            ) -> ConductorResult<()>;

            fn sync_install_dna(&self, dna: DnaFile) -> ConductorResult<()>;

            fn sync_list_dnas(&self) -> ConductorResult<Vec<DnaHash>>;

            fn sync_get_dna(&self, hash: DnaHash) -> Option<DnaFile>;

            fn sync_invoke_zome(
                &self,
                invocation: ZomeInvocation,
            ) -> ConductorApiResult<ZomeInvocationResult>;

            fn sync_autonomic_cue(&self, cue: AutonomicCue, cell_id: &CellId) -> ConductorApiResult<()>;

            fn sync_get_wait_handle(&self) -> Option<TaskManagerRunHandle>;

            fn sync_get_arbitrary_admin_websocket_port(&self) -> Option<u16>;

            fn sync_shutdown(&self);

            fn sync_keystore(&self) -> &KeystoreSender;

            fn sync_create_cells(
                &self,
                cell_ids: Vec<CellId>,
                handle: ConductorHandle,
            ) -> ConductorResult<()>;

            fn sync_get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite>;
        }

        trait Clone {
            fn clone(&self) -> Self;
        }
    }

    #[async_trait::async_trait]
    impl ConductorHandleT for MockConductorHandle {
        async fn check_running(&self) -> ConductorResult<()> {
            self.sync_check_running()
        }

        async fn add_admin_interfaces_via_handle(
            &self,
            handle: ConductorHandle,
            configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<()> {
            self.sync_add_admin_interfaces_via_handle(handle, configs)
        }

        async fn install_dna(&self, dna: DnaFile) -> ConductorResult<()> {
            self.sync_install_dna(dna)
        }

        async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
            self.sync_list_dnas()
        }

        async fn get_dna(&self, hash: DnaHash) -> Option<DnaFile> {
            self.sync_get_dna(hash)
        }

        async fn invoke_zome(
            &self,
            invocation: ZomeInvocation,
        ) -> ConductorApiResult<ZomeInvocationResult> {
            self.sync_invoke_zome(invocation)
        }

        async fn autonomic_cue(
            &self,
            cue: AutonomicCue,
            cell_id: &CellId,
        ) -> ConductorApiResult<()> {
            self.sync_autonomic_cue(cue, cell_id)
        }

        async fn get_wait_handle(&self) -> Option<TaskManagerRunHandle> {
            self.sync_get_wait_handle()
        }

        async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
            self.sync_get_arbitrary_admin_websocket_port()
        }

        async fn shutdown(&self) {
            self.sync_shutdown()
        }

        fn keystore(&self) -> &KeystoreSender {
            self.sync_keystore()
        }

        async fn create_cells(
            &self,
            cell_ids: Vec<CellId>,
            handle: ConductorHandle,
        ) -> ConductorResult<()> {
            self.sync_create_cells(cell_ids, handle)
        }

        async fn get_cell_env(&self, cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite> {
            self.sync_get_cell_env(cell_id)
        }
    }
}
