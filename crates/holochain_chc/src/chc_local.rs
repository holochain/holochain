use std::sync::Arc;

use crate::*;
use holochain_keystore::MetaLairClient;

/// Mutable wrapper around local CHC
pub struct ChcLocal {
    inner: parking_lot::Mutex<ChcLocalInner>,
    keystore: MetaLairClient,
    agent: AgentPubKey,
}

impl ChcLocal {
    /// Constructor
    pub fn new(keystore: MetaLairClient, agent: AgentPubKey) -> Self {
        Self {
            inner: parking_lot::Mutex::new(Default::default()),
            keystore,
            agent,
        }
    }
}

/// A local Rust implementation of a CHC, for testing purposes only.
#[derive(Default)]
pub struct ChcLocalInner {
    records: Vec<RecordItem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RecordItem {
    /// The action
    action: SignedActionHashed,

    /// The entry, encrypted (TODO: by which key?), with the signature
    /// of the encrypted bytes
    pub encrypted_entry: Option<(Arc<EncryptedEntry>, Signature)>,
}

#[async_trait::async_trait]
impl ChainHeadCoordinator for ChcLocal {
    type Item = SignedActionHashed;

    async fn add_records_request(&self, request: AddRecordsRequest) -> ChcResult<()> {
        let mut m = self.inner.lock();
        let head = m
            .records
            .last()
            .map(|r| (r.action.get_hash().clone(), r.action.seq()));
        let records: Vec<_> = request
            .into_iter()
            .map(|r| {
                let signed_action: SignedActionHashed =
                    holochain_serialized_bytes::decode(&r.signed_action_msgpack).unwrap();

                RecordItem {
                    action: signed_action,
                    encrypted_entry: r.encrypted_entry,
                }
            })
            .collect();
        let actions = records.iter().map(|r| &r.action);
        validate_chain(actions, &head).map_err(|_| {
            let (hash, seq) = head.unwrap();
            ChcError::InvalidChain(seq, hash)
        })?;
        m.records.extend(records);
        Ok(())
    }

    async fn get_record_data_request(
        &self,
        request: GetRecordsRequest,
    ) -> ChcResult<Vec<(SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>)>> {
        let m = self.inner.lock();
        let records = if let Some(hash) = request.payload.since_hash.as_ref() {
            m.records
                .iter()
                .skip_while(|r| hash != r.action.get_hash())
                .skip(1)
                .cloned()
                .collect()
        } else {
            m.records.clone()
        };
        Ok(records
            .into_iter()
            .map(
                |RecordItem {
                     action,
                     encrypted_entry,
                 }| (action, encrypted_entry),
            )
            .collect())
    }
}

impl ChainHeadCoordinatorExt for ChcLocal {
    fn signing_info(&self) -> (MetaLairClient, AgentPubKey) {
        (self.keystore.clone(), self.agent.clone())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use holochain_types::test_utils::valid_arbitrary_chain;
    use ChainHeadCoordinatorExt;

    use pretty_assertions::assert_eq;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_add_records_local() {
        let mut g = random_generator();
        let keystore = holochain_keystore::test_keystore();
        let agent = fake_agent_pubkey_1();
        let chc = Arc::new(ChcLocal::new(keystore.clone(), agent.clone()));

        assert_eq!(chc.clone().head().await.unwrap(), None);

        let chain = valid_arbitrary_chain(&mut g, keystore, agent, 20).await;
        let hash = |i: usize| chain[i].action_address().clone();

        let t0 = &chain[0..3];
        let t1 = &chain[3..6];
        let t2 = &chain[6..9];
        let t11 = &chain[11..=11];

        chc.clone().add_records(t0.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(2));
        chc.clone().add_records(t1.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        // last_hash doesn't match
        assert!(chc.clone().add_records(t0.to_vec()).await.is_err());
        assert!(chc.clone().add_records(t1.to_vec()).await.is_err());
        assert!(chc.clone().add_records(t11.to_vec()).await.is_err());
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(5));

        chc.clone().add_records(t2.to_vec()).await.unwrap();
        assert_eq!(chc.clone().head().await.unwrap().unwrap(), hash(8));

        assert_eq!(
            chc.clone().get_record_data(None).await.unwrap(),
            &chain[0..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(0))).await.unwrap(),
            &chain[1..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(3))).await.unwrap(),
            &chain[4..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(7))).await.unwrap(),
            &chain[8..9]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(8))).await.unwrap(),
            &[]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(9))).await.unwrap(),
            &[]
        );
        assert_eq!(
            chc.clone().get_record_data(Some(hash(13))).await.unwrap(),
            &[]
        );
    }
}
