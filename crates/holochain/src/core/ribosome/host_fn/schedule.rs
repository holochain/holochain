use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;
use holochain_types::prelude::*;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: String,
) -> Result<(), WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ write_workspace: Permission::Allow, .. } => {
            call_context.host_context().workspace().source_chain().scratch().apply(|scratch| {
                scratch.add_scheduled_fn(ScheduledFn::new(call_context.zome.zome_name().clone(), input.into()));
            }).map_err(|e| WasmError::Host(e.to_string()))?;
            Ok(())
        },
        _ => unreachable!(),
    }
}

#[cfg(test)]
pub mod tests {
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use crate::sweettest::SweetDnaFile;
    use holochain_types::prelude::AgentPubKeyFixturator;
    use crate::core::ribosome::MockDnaStore;
    use hdk::prelude::*;
    use crate::sweettest::SweetConductor;
    use crate::conductor::ConductorBuilder;
    use holochain_state::schedule::fn_is_scheduled;
    use holochain_state::prelude::schedule_fn;
    use rusqlite::Transaction;
    use holochain_state::prelude::*;
    use holochain_state::schedule::live_scheduled_fns;

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn schedule_test_low_level() -> anyhow::Result<()> {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Schedule])
            .await
            .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store
            .expect_get_entry_def()
            .return_const(EntryDef::default_with_id("thing"));

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let _apps = conductor
        .setup_app_for_agents(
            "app-",
            &[alice_pubkey.clone(), bob_pubkey.clone()],
            &[dna_file.into()],
        )
        .await
        .unwrap();

        let cell_id = conductor.handle().list_cell_ids(None)[0].clone();
        let cell_env = conductor.handle().get_cell_env(&cell_id).unwrap();

        cell_env.async_commit(move |txn: &mut Transaction| {
            let now = Timestamp::now();
            let the_past = (now - std::time::Duration::from_millis(1)).unwrap();
            let the_future = (now + std::time::Duration::from_millis(1000)).unwrap();
            let the_distant_future = (now + std::time::Duration::from_millis(2000)).unwrap();

            let ephemeral_scheduled_fn = ScheduledFn::new("foo".into(), "bar".into());
            let persisted_scheduled_fn = ScheduledFn::new("1".into(), "2".into());
            let persisted_schedule = Schedule::Persisted("* * * * * * * ".into());

            schedule_fn(txn, persisted_scheduled_fn.clone(), Some(persisted_schedule.clone()), now).unwrap();
            schedule_fn(txn, ephemeral_scheduled_fn.clone(), None, now).unwrap();

            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone()).unwrap());
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());

            // Deleting live ephemeral scheduled fns from now should delete.
            delete_live_ephemeral_scheduled_fns(txn, now).unwrap();
            assert!(!fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone()).unwrap());

            schedule_fn(txn, ephemeral_scheduled_fn.clone(), None, now).unwrap();
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone()).unwrap());

            // Deleting live ephemeral fns from a past time should do nothing.
            delete_live_ephemeral_scheduled_fns(txn, the_past).unwrap();
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone()).unwrap());

            // Deleting live ephemeral fns from the future should delete.
            delete_live_ephemeral_scheduled_fns(txn, the_future).unwrap();
            assert!(!fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone()).unwrap());

            // Deleting all ephemeral fns should delete.
            schedule_fn(txn, ephemeral_scheduled_fn.clone(), None, now).unwrap();
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());
            delete_all_ephemeral_scheduled_fns(txn).unwrap();
            assert!(!fn_is_scheduled(txn, ephemeral_scheduled_fn.clone()).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone()).unwrap());

            let ephemeral_future_schedule = Schedule::Ephemeral(std::time::Duration::from_millis(1001));
            schedule_fn(
                txn,
                ephemeral_scheduled_fn.clone(),
                Some(ephemeral_future_schedule.clone()),
                now
            ).unwrap();
            assert_eq!(
                vec![
                    (persisted_scheduled_fn.clone(), Some(persisted_schedule.clone()))
                ],
                live_scheduled_fns(txn, the_future).unwrap(),
            );
            assert_eq!(
                vec![
                    (persisted_scheduled_fn, Some(persisted_schedule)),
                    (ephemeral_scheduled_fn, Some(ephemeral_future_schedule)),
                ],
                live_scheduled_fns(txn, the_distant_future).unwrap(),
            );

            Result::<(), DatabaseError>::Ok(())
        }).await.unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn schedule_test() -> anyhow::Result<()> {
        observability::test_run().ok();
        let (dna_file, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Schedule])
            .await
            .unwrap();

        let alice_pubkey = fixt!(AgentPubKey, Predictable, 0);
        let bob_pubkey = fixt!(AgentPubKey, Predictable, 1);

        let mut dna_store = MockDnaStore::new();
        dna_store.expect_add_dnas::<Vec<_>>().return_const(());
        dna_store.expect_add_entry_defs::<Vec<_>>().return_const(());
        dna_store.expect_add_dna().return_const(());
        dna_store
            .expect_get()
            .return_const(Some(dna_file.clone().into()));
        dna_store
            .expect_get_entry_def()
            .return_const(EntryDef::default_with_id("thing"));

        let mut conductor =
            SweetConductor::from_builder(ConductorBuilder::with_mock_dna_store(dna_store)).await;

        let apps = conductor
            .setup_app_for_agents(
                "app-",
                &[alice_pubkey.clone(), bob_pubkey.clone()],
                &[dna_file.into()],
            )
            .await
            .unwrap();

        let ((alice,), (bobbo,)) = apps.into_tuples();
        let alice = alice.zome(TestWasm::Schedule);
        let bobbo = bobbo.zome(TestWasm::Schedule);

        // Let's just drive alice to exhaust all ticks.
        let _schedule: () = conductor
            .call(
                &alice,
                "schedule",
                ()
            )
            .await;
        let mut i: usize = 0;
        while i < 10 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            conductor.handle().dispatch_scheduled_fns().await;
            i = i + 1;
        }
        let query_tick: Vec<Element> = conductor
        .call(
            &alice,
            "query_tick",
            ()
        )
        .await;
        assert_eq!(query_tick.len(), 5);

        // The persistent schedule should run once in second.
        let query_tock: Vec<Element> = conductor
            .call(
                &alice,
                "query_tock",
                ()
            ).await;
        assert!(query_tock.len() < 3);

        // If Bob does a few ticks and then calls `start_scheduler` the
        // ephemeral scheduled task will be flushed so the ticks will not be
        // exhaused until the function is rescheduled.
        let _shedule: () = conductor
            .call(
                &bobbo,
                "schedule",
                ()
            ).await;
        conductor.handle().dispatch_scheduled_fns().await;
        let query1: Vec<Element> = conductor
            .call(
                &bobbo,
                "query_tick",
                ()
            ).await;
        assert_eq!(query1.len(), 1);
        conductor.handle().dispatch_scheduled_fns().await;
        let query2: Vec<Element> = conductor
            .call(
                &bobbo,
                "query_tick",
                ()
            ).await;
        assert_eq!(query2.len(), query1.len() + 1);

        // With a fast scheduler bob should clear everything out.
        let _ = conductor.clone().start_scheduler(std::time::Duration::from_millis(1)).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let q: Vec<Element> = conductor
        .call(
            &bobbo,
            "query_tick",
            ()
        )
        .await;
        assert_eq!(q.len(), query2.len());

        // Rescheduling will allow bob catch up to alice.
        let _shedule: () = conductor
            .call(
                &bobbo,
                "schedule",
                ()
            ).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let q2: Vec<Element> = conductor
            .call(
                &bobbo,
                "query_tick",
                ()
            )
            .await;
        assert_eq!(q2.len(), 5);

        Ok(())
    }
}