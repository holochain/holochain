//! # Cascade
//! This module is still a work in progress.
//! Here is some pseudocode we are using to build it.
//! ## Dimensions
//! get vs get_links
//! default vs options
//! fast vs strict (is set by app dev)
//!
//! ## Get
//! ### Default - Get's the latest version
//! Scratch Live -> Return
//! Scratch NotInCascade -> Goto Cas
//! Scratch _ -> None
//! Cas Live -> Return
//! Cas NotInCascade -> Goto cache
//! Cas _ -> None
//! Cache Live -> Return
//! Cache Pending -> Goto Network
//! Cache NotInCascade -> Goto Network
//! Cache _ -> None
//!
//! ## Get Links
//! ### Default - Get's the latest version
//! if I'm an authority
//! Scratch Found-> Return
//! Scratch NotInCascade -> Goto Cas
//! Cas Found -> Return
//! Cas NotInCascade -> Goto Network
//! else
//! Network Found -> Return
//! Network NotInCascade -> Goto Cache
//! Cache Found -> Return
//! Cache NotInCascade -> None
//!
//! ## Pagination
//! gets most recent N links with default N (50)
//! Page number
//! ## Loading
//! load_true loads the results into cache

use super::{
    chain_cas::ChainCasBuf,
    chain_meta::{ChainMetaBuf, ChainMetaBufT, EntryDhtStatus},
};
use holochain_state::{error::DatabaseResult, prelude::Reader};
use holochain_types::entry::Entry;
use holochain_types::entry::EntryAddress;
use std::collections::HashSet;
use tracing::*;

#[cfg(test)]
mod test;

pub struct Cascade<'env, C = ChainMetaBuf<'env, ()>>
where
    C: ChainMetaBufT<'env>,
{
    primary: &'env ChainCasBuf<'env, Reader<'env>>,
    primary_meta: &'env C,

    cache: &'env ChainCasBuf<'env, Reader<'env>>,
    cache_meta: &'env C,
}

/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(Entry),
    /// We haven't found the entry yet and should
    /// continue searching down the cascade
    Continue,
    /// We haven't found the entry and should
    /// not continue searching down the cascade
    // TODO This information is currently not passed back to
    // the caller however it might be useful.
    NotInCascade,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env, C> Cascade<'env, C>
where
    C: ChainMetaBufT<'env>,
{
    /// Constructs a [Cascade], taking references to a CAS and a cache
    pub fn new(
        primary: &'env ChainCasBuf<'env, Reader<'env>>,
        primary_meta: &'env C,
        cache: &'env ChainCasBuf<'env, Reader<'env>>,
        cache_meta: &'env C,
    ) -> Self {
        Cascade {
            primary,
            primary_meta,
            cache,
            cache_meta,
        }
    }

    #[instrument(skip(self))]
    /// Gets an entry from the cas or cache depending on it's metadata
    // TODO asyncify slow blocking functions here
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get(&self, entry_address: EntryAddress) -> DatabaseResult<Option<Entry>> {
        // Cas
        let search = self
            .primary
            .get_entry(entry_address.clone())?
            .and_then(|entry| {
                self.primary_meta
                    .get_crud(entry_address.clone())
                    .ok()
                    .map(|crud| {
                        if let EntryDhtStatus::Live = crud {
                            Search::Found(entry)
                        } else {
                            Search::NotInCascade
                        }
                    })
            })
            .unwrap_or_else(|| Search::Continue);

        // Cache
        match search {
            Search::Continue => {
                Ok(self
                    .cache
                    .get_entry(entry_address.clone())?
                    .and_then(|entry| {
                        self.cache_meta
                            .get_crud(entry_address)
                            .ok()
                            .and_then(|crud| match crud {
                                EntryDhtStatus::Live => Some(entry),
                                _ => None,
                            })
                    }))
            }
            Search::Found(entry) => Ok(Some(entry)),
            Search::NotInCascade => Ok(None),
        }
    }

    /// Gets an links from the cas or cache depending on it's metadata
    // TODO asyncify slow blocking functions here
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get_links<S: Into<String>>(
        &self,
        base: EntryAddress,
        tag: S,
    ) -> DatabaseResult<HashSet<EntryAddress>> {
        // Am I an authority?
        let authority = self.primary.contains(base.clone())?;
        let tag = tag.into();
        if authority {
            // Cas
            let links = self.primary_meta.get_links(base.clone(), tag.clone())?;

            // Cache
            if links.is_empty() {
                self.cache_meta.get_links(base, tag)
            } else {
                Ok(links)
            }
        } else {
            // Cache
            self.cache_meta.get_links(base, tag)
        }
    }
}

pub mod raw {
    use super::*;
    // TODO write tests to varify the invariant.
    /// This is needed to use the database where
    /// the lifetimes cannot be verified by
    /// the compiler (e.g. with wasmer).
    /// The checks are moved to runtime.
    /// The api is non-blocking because this
    /// should never be contested if the invariant is held.
    /// This type cannot write to the db.
    /// It only takes a [Reader].
    /// It only needs read access to the scratch space
    pub struct UnsafeCascade {
        cascade: std::sync::Weak<std::sync::RwLock<*const std::ffi::c_void>>,
    }

    // TODO: SAFETY: Tie the guard to the lmdb `'env` lifetime.
    /// If this guard is dropped the underlying
    /// ptr cannot be used.
    /// ## Safety
    /// Don't use `mem::forget` on this type as it will
    /// break the checks.
    pub struct UnsafeCascadeGuard {
        cascade: Option<std::sync::Arc<std::sync::RwLock<*const std::ffi::c_void>>>,
    }

    impl UnsafeCascade {
        pub fn from_ref<'env, C>(cascade: &Cascade<'env, C>) -> (UnsafeCascadeGuard, Self)
        where
            C: ChainMetaBufT<'env>,
        {
            let raw_ptr = cascade as *const Cascade<C> as *const std::ffi::c_void;
            let guard = std::sync::Arc::new(std::sync::RwLock::new(raw_ptr));
            let cascade = std::sync::Arc::downgrade(&guard);
            let guard = UnsafeCascadeGuard {
                cascade: Some(guard),
            };
            let cascade = Self { cascade };
            (guard, cascade)
        }

        // Make this test only when the aliasing issue is fixed
        //#[cfg(test)]
        /// Useful when we need this type for tests where we don't want to use it.
        /// It will always return None.
        pub fn test() -> Self {
            let fake_ptr = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr()
                as *const std::ffi::c_void;
            let guard = std::sync::Arc::new(std::sync::RwLock::new(fake_ptr));
            let cascade = std::sync::Arc::downgrade(&guard);
            // Make sure the weak Arc cannot be upgraded
            std::mem::drop(guard);
            Self { cascade }
        }

        pub unsafe fn apply_ref<
            R: 'static,
            F: FnOnce(&Cascade) -> R,
        >(
            &self,
            f: F,
        ) -> Option<R> {
            // Check it exists
            self.cascade
                .upgrade()
                // Check that no-one else can write
                .and_then(|lock| {
                    lock.try_read().ok().and_then(|guard| {
                        let sc = *guard as *const Cascade;
                        sc.as_ref().map(|s| f(s))
                    })
                })
        }
    }

    impl Drop for UnsafeCascadeGuard {
        fn drop(&mut self) {
            std::sync::Arc::try_unwrap(self.cascade.take().expect("BUG: This has to be here"))
                .expect("BUG: Invariant broken, strong reference active while guard is dropped");
        }
    }
}
