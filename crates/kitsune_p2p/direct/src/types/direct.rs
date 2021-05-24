//! kdirect entrypoint type

use crate::*;
use futures::future::BoxFuture;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::future::Future;

/// Trait representing a kitsune direct api implementation
pub trait AsKitsuneDirect: 'static + Send + Sync {
    /// Get a uniq val that assists with Eq/Hash of trait objects.
    fn uniq(&self) -> Uniq;

    /// Check if this kdirect instance has been closed
    fn is_closed(&self) -> bool;

    /// Explicitly close this kdirect instance
    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()>;

    /// Get a handle to the persist store used by this kdirect instance.
    fn get_persist(&self) -> KdPersist;

    /// Get the local ui address of this instance.
    fn get_ui_addr(&self) -> KitsuneResult<std::net::SocketAddr>;

    /// List transport bindings
    fn list_transport_bindings(&self) -> BoxFuture<'static, KitsuneResult<Vec<TxUrl>>>;

    /// Bind a local control handle to this instance
    fn bind_control_handle(&self) -> BoxFuture<'static, KitsuneResult<(KdHnd, KdHndEvtStream)>>;
}

/// the driver future type for the kitsune direct instance
pub type KitsuneDirectDriver = BoxFuture<'static, ()>;

/// Struct representing a kitsune direct api implementation
#[derive(Clone)]
pub struct KitsuneDirect(pub Arc<dyn AsKitsuneDirect>);

impl PartialEq for KitsuneDirect {
    fn eq(&self, oth: &Self) -> bool {
        self.0.uniq().eq(&oth.0.uniq())
    }
}

impl Eq for KitsuneDirect {}

impl std::hash::Hash for KitsuneDirect {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uniq().hash(state)
    }
}

impl KitsuneDirect {
    /// Check if this kdirect instance has been closed
    pub fn is_closed(&self) -> bool {
        AsKitsuneDirect::is_closed(&*self.0)
    }

    /// Explicitly close this kdirect instance
    pub fn close(&self, code: u32, reason: &str) -> impl Future<Output = ()> + 'static + Send {
        AsKitsuneDirect::close(&*self.0, code, reason)
    }

    /// Get a handle to the persist store used by this kdirect instance.
    /// (persist is closed separately, as we may have cleanup
    /// operations to do on the store.)
    pub fn get_persist(&self) -> KdPersist {
        AsKitsuneDirect::get_persist(&*self.0)
    }

    /// Get the local ui address of this instance.
    pub fn get_ui_addr(&self) -> KitsuneResult<std::net::SocketAddr> {
        AsKitsuneDirect::get_ui_addr(&*self.0)
    }

    /// List transport bindings
    pub fn list_transport_bindings(
        &self,
    ) -> impl Future<Output = KitsuneResult<Vec<TxUrl>>> + 'static + Send {
        AsKitsuneDirect::list_transport_bindings(&*self.0)
    }

    /// Bind a local control handle to this instance
    pub fn bind_control_handle(
        &self,
    ) -> impl Future<Output = KitsuneResult<(KdHnd, KdHndEvtStream)>> + 'static + Send {
        AsKitsuneDirect::bind_control_handle(&*self.0)
    }
}
