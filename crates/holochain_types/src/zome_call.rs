//! Data needed to make zome calls.
use crate::prelude::*;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use std::sync::Arc;

impl ZomeCallUnsigned {


    /// Sign the unsigned zome call in a canonical way to produce a signature.
    pub async fn sign(&self, keystore: &MetaLairClient) -> LairResult<Signature> {
        self.provenance
            .sign_raw(keystore, self.data_to_sign().map_err(|e| e.to_string())?)
            .await
    }
}
