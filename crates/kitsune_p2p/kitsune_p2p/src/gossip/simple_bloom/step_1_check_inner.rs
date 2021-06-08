use crate::*;
use observability::tracing;

use super::{CheckResult, SimpleBloomMod};

impl SimpleBloomMod {
    pub(super) async fn step_1_check_inner(&self) -> KitsuneP2pResult<CheckResult> {
        let (not_ready, tgt) = self.inner.share_mut(|i, _| {
            // first, if we don't have any local agents, there's
            // no point in doing any gossip logic
            let not_ready = i.local_agents.is_empty();
            let tgt = i.initiate_tgt.clone();
            Ok((not_ready, tgt))
        })?;
        if not_ready {
            return Ok(CheckResult::NotReady);
        }

        // next, check to see if we should time out any current initiate_tgt
        if let Some(initiate_tgt) = tgt {
            if let Some(metric) = self.get_metric(initiate_tgt.agents().clone()).await? {
                if metric.was_err
                    || metric.last_touch.elapsed()?.as_millis() as u32
                        > self.tuning_params.gossip_peer_on_success_next_gossip_delay_ms
                        // give us a little leeway... we don't
                        // need to be too agressive with timing out
                        // this loop
                        * 2
                {
                    tracing::warn!("gossip timeout on initiate tgt {:?}", initiate_tgt);

                    self.inner.share_mut(|i, _| {
                        i.initiate_tgt = None;
                        Ok(())
                    })?;
                } else {
                    // we're still processing the current initiate...
                    // don't bother syncing locally
                    return Ok(CheckResult::SkipSyncAndInitiate);
                }
            } else {
                // erm... we have an initate tgt,
                // but we've never seen them??
                // this must be a logic error.
                unreachable!()
            }
        }
        // TODO: clean up ugly locking here
        let needs_sync = self.inner.share_mut(|i, _| {
            Ok(i.initiate_tgt.is_none()
                && i.last_initiate_check.elapsed().as_millis() as u32
                    > self.tuning_params.gossip_loop_iteration_delay_ms)
        })?;
        if needs_sync {
            Ok(CheckResult::SyncAndInitiate)
        } else {
            Ok(CheckResult::SkipSyncAndInitiate)
        }
    }
}
