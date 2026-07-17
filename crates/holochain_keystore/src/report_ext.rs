//! Extension trait definition [`ReportEntryFetchedOpsExt`].

use crate::AgentPubKeyExt;
use base64::prelude::*;
use holo_hash::AgentPubKey;
use holochain_types::report::ReportEntryFetchedOps;
use must_future::MustBoxFuture;

/// Extension for keystore operations on a [`ReportEntryFetchedOps`].
pub trait ReportEntryFetchedOpsExt {
    /// Verify the signatures.
    fn verify(&self) -> MustBoxFuture<'static, bool>;
}

impl ReportEntryFetchedOpsExt for ReportEntryFetchedOps {
    fn verify(&self) -> MustBoxFuture<'static, bool> {
        let to_verify = self.encode_for_verification();

        tracing::trace!(to_verify = %String::from_utf8_lossy(&to_verify), report = ?self, "verify");

        if self.agent_pubkeys.is_empty() || self.agent_pubkeys.len() != self.signatures.len() {
            tracing::trace!("report signatures invalid");
            return MustBoxFuture::new(async move { false });
        }

        let iter = self
            .agent_pubkeys
            .iter()
            .cloned()
            .zip(self.signatures.iter().cloned())
            .collect::<Vec<_>>();

        MustBoxFuture::new(async move {
            for (agent, sig) in iter {
                let agent = agent.trim_start_matches("u");
                let agent = match BASE64_URL_SAFE_NO_PAD.decode(agent) {
                    Ok(agent) => agent,
                    Err(_) => return false,
                };
                if agent.len() != 39 {
                    return false;
                }
                let sig = match BASE64_URL_SAFE_NO_PAD.decode(sig) {
                    Ok(sig) => sig,
                    Err(_) => return false,
                };
                if sig.len() != 64 {
                    return false;
                }
                let sig: [u8; 64] = sig.try_into().expect("array conversion failed");

                let pk = AgentPubKey::from_raw_39(agent);
                match pk
                    .verify_signature_raw(&sig.into(), to_verify.clone().into())
                    .await
                {
                    Ok(true) => (),
                    _ => return false,
                }
            }

            true
        })
    }
}
