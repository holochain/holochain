use bytes::Bytes;
use holo_hash::AgentPubKey;
use holochain_keystore::MetaLairClient;
use kitsune2_api::{AgentId, AgentInfo, BoxFut, DhtArc, K2Error, K2Result, LocalAgent, Signer};
use lair_keystore_api::dependencies::parking_lot;
use parking_lot::Mutex;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

fn apply_arc_factor(arc: DhtArc, factor: u32) -> DhtArc {
    match arc {
        DhtArc::Empty => DhtArc::Empty,
        DhtArc::Arc(start, _end) => {
            let len = (arc.arc_span() as u64 + 1).saturating_mul(factor as u64);

            if len == 0 {
                return DhtArc::Empty;
            }

            let span = if len >= u32::MAX as u64 {
                u32::MAX
            } else {
                (len - 1) as u32
            };

            DhtArc::Arc(
                start,
                (std::num::Wrapping(start) + std::num::Wrapping(span)).0,
            )
        }
    }
}

struct LocalAgentInner {
    callback: Option<Arc<dyn Fn() + 'static + Send + Sync>>,
    /// The storage arc that the agent is currently claiming authority over.
    storage_arc: DhtArc,
    /// The storage arc that the agent is trying to cover.
    target_arc: DhtArc,
}

/// Holochain implementation of a Kitsune2 [LocalAgent] and [Signer].
pub struct HolochainP2pLocalAgent {
    /// The Holochain [AgentPubKey] for this local agent.
    agent: AgentPubKey,
    /// The [AgentId] derived from the [AgentPubKey], as the 32-byte key and 4-byte location.
    agent_id: AgentId,
    /// A [MetaLairClient] to allow this agent to sign messages.
    keystore_client: MetaLairClient,
    /// The inner state that can be modified during the lifecycle of the agent
    inner: Mutex<LocalAgentInner>,
    /// The target arc factor to apply to hints.
    target_arc_factor: u32,
}

impl HolochainP2pLocalAgent {
    /// Create a new [HolochainP2pLocalAgent].
    pub fn new(
        agent: AgentPubKey,
        initial_target_arc: DhtArc,
        initial_target_arc_factor: u32,
        client: MetaLairClient,
    ) -> Self {
        let agent_id = agent.to_k2_agent();
        Self {
            agent,
            agent_id,
            keystore_client: client,
            inner: Mutex::new(LocalAgentInner {
                callback: None,
                storage_arc: DhtArc::Empty,
                target_arc: apply_arc_factor(initial_target_arc, initial_target_arc_factor),
            }),
            target_arc_factor: initial_target_arc_factor,
        }
    }
}

impl Debug for HolochainP2pLocalAgent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainP2pLocalAgent")
            .field("agent", &self.agent)
            .finish()
    }
}

impl Signer for HolochainP2pLocalAgent {
    fn sign<'a, 'b: 'a, 'c: 'a>(
        &'a self,
        _agent_info: &'b AgentInfo,
        message: &'c [u8],
    ) -> BoxFut<'a, K2Result<Bytes>> {
        Box::pin(async move {
            let out = self
                .keystore_client
                .sign(self.agent.clone(), message.into())
                .await
                .map_err(|e| K2Error::other_src("Failed to sign message", e))?;

            Ok(Bytes::copy_from_slice(&out.0))
        })
    }
}

impl LocalAgent for HolochainP2pLocalAgent {
    fn agent(&self) -> &AgentId {
        &self.agent_id
    }

    fn register_cb(&self, cb: Arc<dyn Fn() + 'static + Send + Sync>) {
        self.inner.lock().callback = Some(cb);
    }

    fn invoke_cb(&self) {
        if let Some(cb) = self.inner.lock().callback.clone() {
            cb();
        }
    }

    fn get_cur_storage_arc(&self) -> DhtArc {
        self.inner.lock().storage_arc
    }

    fn set_cur_storage_arc(&self, arc: DhtArc) {
        self.inner.lock().storage_arc = arc;
    }

    fn get_tgt_storage_arc(&self) -> DhtArc {
        self.inner.lock().target_arc
    }

    fn set_tgt_storage_arc_hint(&self, arc: DhtArc) {
        let factor = if self.target_arc_factor > 1 {
            tracing::error!("Received target arc factor > 1, this is not yet allowed until sharding is implemented!");
            1
        } else {
            self.target_arc_factor
        };

        self.inner.lock().target_arc = apply_arc_factor(arc, factor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_arc_factor_fixture() {
        for (expect, orig, factor) in [
            (DhtArc::Empty, DhtArc::Empty, 0),
            (DhtArc::Empty, DhtArc::Empty, 27),
            (DhtArc::Empty, DhtArc::Empty, u32::MAX),
            (DhtArc::Arc(0, 0), DhtArc::Arc(0, 0), 1),
            (DhtArc::Arc(0, 1), DhtArc::Arc(0, 1), 1),
            (DhtArc::Arc(u32::MAX, 0), DhtArc::Arc(u32::MAX, 0), 1),
            (DhtArc::Arc(0, 1), DhtArc::Arc(0, 0), 2),
            (DhtArc::Arc(13, 522), DhtArc::Arc(13, 42), 17),
            (
                DhtArc::Arc(u32::MAX - 7, 110),
                DhtArc::Arc(u32::MAX - 7, u32::MAX - 1),
                17,
            ),
            (DhtArc::Arc(u32::MAX, 0), DhtArc::Arc(u32::MAX, u32::MAX), 2),
            (DhtArc::Arc(0, u32::MAX), DhtArc::Arc(0, 0), u32::MAX),
            (
                DhtArc::Arc(u32::MAX, u32::MAX - 1),
                DhtArc::Arc(u32::MAX, u32::MAX),
                u32::MAX,
            ),
        ] {
            assert_eq!(expect, apply_arc_factor(orig, factor));
        }
    }
}
