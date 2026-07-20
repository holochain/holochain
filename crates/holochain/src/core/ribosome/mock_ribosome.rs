use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::genesis_self_check::GenesisSelfCheckResult;
use crate::core::ribosome::{CallContext, Invocation, MockRibosomeImplT, Ribosome};
use hdk::prelude::{DnaModifiers, ValidateCallbackResult};
use holochain_zome_types::prelude::{
    DnaDefBuilder, DnaDefHashed, ExternIO, FunctionName, InitCallbackResult, NetworkSeed,
};
use mockall::predicate::{always, eq};
use std::sync::Arc;

/// A helper type for working with a [`MockRibosomeImplT`].
pub struct MockRibosomeBuilder {
    mock: MockRibosomeImplT,

    dna_def: DnaDefHashed,

    num_entry_types: i32,

    num_link_types: i32,
}

impl Default for MockRibosomeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRibosomeBuilder {
    /// Create a new builder instance.
    pub fn new() -> Self {
        let dna_def = DnaDefBuilder::default()
            .modifiers(DnaModifiers {
                network_seed: NetworkSeed::default(),
                properties: ().try_into().unwrap(),
            })
            .integrity_zomes(vec![])
            .coordinator_zomes(vec![])
            .build()
            .unwrap();
        Self::new_with_dna_def(DnaDefHashed::from_content_sync(dna_def))
    }

    /// Create a new builder instance with a specified [`DnaDefHashed`] instead of an empty one.
    pub fn new_with_dna_def(dna_def: DnaDefHashed) -> Self {
        Self {
            mock: MockRibosomeImplT::default(),
            dna_def,
            num_entry_types: 0,
            num_link_types: 0,
        }
    }

    /// Specify the value that will be returned by the const function `__num_entry_types`.
    pub fn with_num_entry_types(mut self, num_entry_types: i32) -> Self {
        self.num_entry_types = num_entry_types;
        self
    }

    /// Specify the value that will be returned by the const function `__num_link_types`.
    pub fn with_num_link_types(mut self, num_link_types: i32) -> Self {
        self.num_link_types = num_link_types;
        self
    }

    /// Provide a callback handler for the `init` function.
    pub fn with_init_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<InitCallbackResult>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("init".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    /// Provide a callback handler for the `validate` function.
    pub fn with_validate_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<ValidateCallbackResult>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("validate".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    /// Provide a callback handler for the `genesis_self_check_1` function.
    pub fn with_genesis_self_check_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<GenesisSelfCheckResult>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("genesis_self_check_1".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    /// Provide a callback handler for the `post_commit` function.
    pub fn with_post_commit_handler(
        mut self,
        handler: impl FnMut(CallContext, Arc<dyn Invocation>) -> RibosomeResult<()>
            + Clone
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.mock
            .expect_maybe_call()
            .with(
                always(),
                always(),
                always(),
                always(),
                eq::<FunctionName>("post_commit".into()),
                always(),
            )
            .returning(move |_, call, inv, _, _, _| {
                let mut handler = handler.clone();
                Box::pin(async move {
                    handler(call, inv).map(|out| Some(ExternIO::encode(out).unwrap()))
                })
            });
        self
    }

    /// Access the raw mock in the case that one of the `with_` helpers above is not flexible enough.
    pub fn raw_mock(&mut self) -> &mut MockRibosomeImplT {
        &mut self.mock
    }

    /// Prepare the mock for use and pass it to a [`Ribosome`] instance.
    ///
    /// The following steps are taken:
    /// - Implement `__num_entry_types` and `__num_link_types` callbacks.
    /// - Add a handler for zome functions that don't exist to return `Ok(None)`, matching expected
    ///   behavior for real `maybe_call` implementations.
    /// - Builds a new [`Ribosome`] instance using the mock.
    pub async fn build(mut self) -> RibosomeResult<Ribosome> {
        self.mock
            .expect_call_const_fn()
            .with(always(), always(), eq::<String>("__num_entry_types".into()))
            .returning(move |_, _, _| {
                let num_entry_types = self.num_entry_types;
                Box::pin(async move { Ok(Some(num_entry_types)) })
            });

        self.mock
            .expect_call_const_fn()
            .with(always(), always(), eq::<String>("__num_link_types".into()))
            .returning(move |_, _, _| {
                let num_link_types = self.num_link_types;
                Box::pin(async move { Ok(Some(num_link_types)) })
            });

        // After all other expectations have been added, provide a maybe_call expectation that
        // handles missing functions
        self.mock
            .expect_maybe_call()
            .with(always(), always(), always(), always(), always(), always())
            .returning(move |_, _, _, _, _, _| Box::pin(async move { Ok(None) }));

        Ribosome::new(self.dna_def, self.mock).await
    }
}
