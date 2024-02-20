use crate::core::workflow::WorkflowResult;
use holochain_types::prelude::DhtOpHashed;
use holochain_types::share::RwShare;

type InOpBatchSnd = tokio::sync::oneshot::Sender<WorkflowResult<()>>;
type InOpBatchRcv = tokio::sync::oneshot::Receiver<WorkflowResult<()>>;

#[derive(Debug)]
pub struct InOpBatchEntry {
    pub snd: InOpBatchSnd,
    pub request_validation_receipt: bool,
    pub ops: Vec<DhtOpHashed>,
}

/// A batch of incoming ops memory.
#[derive(Clone)]
pub struct IncomingOpsBatch(RwShare<InOpBatch>);

impl IncomingOpsBatch {
    pub fn is_running(&self) -> bool {
        self.0.share_ref(|b| b.is_running)
    }

    /// if result.0.is_none() -- we queued it to send later
    /// if result.0.is_some() -- the batch should be run now
    pub fn check_insert(
        &self,
        request_validation_receipt: bool,
        ops: Vec<DhtOpHashed>,
    ) -> (Option<Vec<InOpBatchEntry>>, InOpBatchRcv) {
        let (snd, rcv) = tokio::sync::oneshot::channel();
        let entry = InOpBatchEntry {
            snd,
            request_validation_receipt,
            ops,
        };
        self.0.share_mut(|batch| {
            if batch.is_running {
                // there is already a batch running, just queue this
                batch.pending.push(entry);
                (None, rcv)
            } else {
                // no batch running, run this (and assert we never collect stragglers)
                assert!(batch.pending.is_empty());
                batch.is_running = true;
                (Some(vec![entry]), rcv)
            }
        })
    }

    /// if result.is_none() -- we are done, end the loop for now
    /// if result.is_some() -- we got more items to process
    pub fn check_end(&self) -> Option<Vec<InOpBatchEntry>> {
        self.0.share_mut(|batch| {
            assert!(batch.is_running);
            let out: Vec<InOpBatchEntry> = batch.pending.drain(..).collect();
            if out.is_empty() {
                // pending was empty, we can end the loop for now
                batch.is_running = false;
                None
            } else {
                // we have some more pending, continue the running loop
                Some(out)
            }
        })
    }
}

#[derive(Default)]
struct InOpBatch {
    is_running: bool,
    pending: Vec<InOpBatchEntry>,
}

impl Default for IncomingOpsBatch {
    fn default() -> Self {
        Self(RwShare::new(InOpBatch::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_batch() {
        let batch = IncomingOpsBatch::default();
        assert!(!batch.is_running());

        batch.check_insert(false, vec![]);
        assert!(batch.is_running());

        let next_content = batch.check_end();
        assert!(next_content.is_none());
    }

    #[test]
    fn adds_to_pending_while_running() {
        let batch = IncomingOpsBatch::default();
        batch.check_insert(false, vec![]);
        assert!(batch.is_running());

        batch.check_insert(false, vec![]);
        assert!(batch.is_running()); // stays running, just to be sure

        let next_content = batch.check_end();
        assert!(next_content.is_some());

        let next_content = batch.check_end();
        assert!(next_content.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn batches_can_await_completion() {
        let batch = IncomingOpsBatch::default();
        let (mut batch_entry, first_recv) = batch.check_insert(false, vec![]);
        assert!(batch.is_running());

        tokio::spawn({
            let batch = batch.clone();
            async move {
                while let Some(entry) = batch_entry {
                    for e in entry {
                        e.snd.send(Ok(())).unwrap();
                    }
                    batch_entry = batch.check_end();
                }
            }
        });

        let (no_content, second_recv) = batch.check_insert(false, vec![]);
        assert!(no_content.is_none());

        first_recv.await.unwrap().unwrap();
        second_recv.await.unwrap().unwrap();
    }

    #[test]
    #[should_panic]
    fn can_crash_by_calling_end_when_not_running() {
        let batch = IncomingOpsBatch::default();
        batch.check_end();
    }
}
