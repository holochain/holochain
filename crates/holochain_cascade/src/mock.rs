//! Test-only helpers for the mockall-generated [`MockCascade`].

use super::{CascadeSource, MockCascade};
use holochain_state::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

impl MockCascade {
    /// Construct a mock which acts as if the given records were part of local storage
    pub fn with_records(records: Vec<Record>) -> Self {
        let mut cascade = Self::default();

        let map: HashMap<AnyDhtHash, Record> = records
            .into_iter()
            .flat_map(|r| {
                let mut items = vec![(r.action_address().clone().into(), r.clone())];
                if let Some(eh) = r.action().entry_hash() {
                    items.push((eh.clone().into(), r))
                }
                items
            })
            .collect();

        let map0 = Arc::new(parking_lot::Mutex::new(map));

        let map = map0.clone();
        cascade
            .expect_retrieve_public_record()
            .returning(move |hash, _| {
                let m = map.lock();
                let result = m.get(&hash).map(|r| (r.clone(), CascadeSource::Local));
                Box::pin(async move { Ok(result) })
            });

        let map = map0.clone();
        cascade.expect_retrieve_action().returning(move |hash, _| {
            let m = map.lock();
            let result = m
                .get(&hash.into())
                .map(|r| (r.signed_action().clone(), CascadeSource::Local));
            Box::pin(async move { Ok(result) })
        });

        let map = map0;
        cascade.expect_retrieve_entry().returning(move |hash, _| {
            let m = map.lock();
            let result = m.get(&hash.into()).map(|r| {
                (
                    EntryHashed::from_content_sync(r.entry().as_option().unwrap().clone()),
                    CascadeSource::Local,
                )
            });
            Box::pin(async move { Ok(result) })
        });

        cascade
    }
}

#[tokio::test]
async fn test_mock_cascade_with_records() {
    use super::Cascade;
    use ::fixt::fixt;
    use holochain_p2p::actor::NetworkRequestOptions;
    let records = vec![fixt!(Record), fixt!(Record), fixt!(Record)];
    let cascade = MockCascade::with_records(records.clone());
    let opts = NetworkRequestOptions::default();
    let (r0, _) = cascade
        .retrieve_public_record(records[0].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r1, _) = cascade
        .retrieve_public_record(records[1].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r2, _) = cascade
        .retrieve_public_record(records[2].action_address().clone().into(), opts)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(records, vec![r0, r1, r2]);
}
