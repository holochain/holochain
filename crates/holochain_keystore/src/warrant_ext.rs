//! Extension trait definition [`WarrantOpExt`].

use crate::{AgentPubKeyExt, LairResult, MetaLairClient};
use futures::FutureExt;
use holochain_types::prelude::WarrantOp;
use holochain_zome_types::prelude::{SignedWarrant, Warrant};
use must_future::MustBoxFuture;

/// Extension for keystore operations on a [`WarrantOp`].
pub trait WarrantOpExt {
    /// Sign the warrant for use as a [`WarrantOp`].
    fn sign(
        keystore: &MetaLairClient,
        warrant: Warrant,
    ) -> MustBoxFuture<'static, LairResult<WarrantOp>>;
}

impl WarrantOpExt for WarrantOp {
    fn sign(
        keystore: &MetaLairClient,
        warrant: Warrant,
    ) -> MustBoxFuture<'static, LairResult<WarrantOp>> {
        let f = warrant
            .author
            .sign(keystore, warrant.clone())
            .map(|res| res.map(|sig| Self::from(SignedWarrant::new(warrant, sig))));
        MustBoxFuture::new(f)
    }
}
