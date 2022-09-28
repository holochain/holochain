use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: String,
) -> Result<(), RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            call_context
                .host_context()
                .workspace_write()
                .source_chain()
                .as_ref()
                .expect("Must have source chain if write_workspace access is given")
                .scratch()
                .apply(|scratch| {
                    scratch.add_scheduled_fn(ScheduledFn::new(
                        call_context.zome.zome_name().clone(),
                        input.into(),
                    ));
                })
                .map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?;
            Ok(())
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "schedule".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
pub mod tests {
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use hdk::prelude::*;
    use holochain_state::prelude::schedule_fn;
    use holochain_state::prelude::*;
    use holochain_state::schedule::fn_is_scheduled;
    use holochain_state::schedule::live_scheduled_fns;
    use holochain_wasm_test_utils::TestWasm;
    use rusqlite::Transaction;

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(feature = "test_utils")]
    async fn schedule_test_low_level() -> anyhow::Result<()> {
        observability::test_run().ok();
        let RibosomeTestFixture {
            alice_pubkey,
            alice_host_fn_caller,
            ..
        } = RibosomeTestFixture::new(TestWasm::Schedule).await;

        alice_host_fn_caller
            .authored_db
            .async_commit(move |txn: &mut Transaction| {
                let now = Timestamp::now();
                let the_past = (now - std::time::Duration::from_millis(1)).unwrap();
                let the_future = (now + std::time::Duration::from_millis(1000)).unwrap();
                let the_distant_future = (now + std::time::Duration::from_millis(2000)).unwrap();

                let ephemeral_scheduled_fn = ScheduledFn::new("foo".into(), "bar".into());
                let persisted_scheduled_fn = ScheduledFn::new("1".into(), "2".into());
                let persisted_schedule = Schedule::Persisted("* * * * * * * ".into());

                schedule_fn(
                    txn,
                    &alice_pubkey,
                    persisted_scheduled_fn.clone(),
                    Some(persisted_schedule.clone()),
                    now,
                )
                .unwrap();
                schedule_fn(
                    txn,
                    &alice_pubkey,
                    ephemeral_scheduled_fn.clone(),
                    None,
                    now,
                )
                .unwrap();

                assert!(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey).unwrap()
                );
                assert!(
                    fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey).unwrap()
                );

                // Deleting live ephemeral scheduled fns from now should delete.
                delete_live_ephemeral_scheduled_fns(txn, now, &alice_pubkey).unwrap();
                assert!(
                    !fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );
                assert!(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );

                schedule_fn(
                    txn,
                    &alice_pubkey,
                    ephemeral_scheduled_fn.clone(),
                    None,
                    now,
                )
                .unwrap();
                assert!(
                    fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );
                assert!(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );

                // Deleting live ephemeral fns from a past time should do nothing.
                delete_live_ephemeral_scheduled_fns(txn, the_past, &alice_pubkey).unwrap();
                assert!(
                    fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );
                assert!(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );

                // Deleting live ephemeral fns from the future should delete.
                delete_live_ephemeral_scheduled_fns(txn, the_future, &alice_pubkey).unwrap();
                assert!(
                    !fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );
                assert!(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );

                // Deleting all ephemeral fns should delete.
                schedule_fn(
                    txn,
                    &alice_pubkey,
                    ephemeral_scheduled_fn.clone(),
                    None,
                    now,
                )
                .unwrap();
                assert!(
                    fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );
                delete_all_ephemeral_scheduled_fns(txn, &alice_pubkey).unwrap();
                assert!(
                    !fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );
                assert!(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap()
                );

                let ephemeral_future_schedule =
                    Schedule::Ephemeral(std::time::Duration::from_millis(1001));
                schedule_fn(
                    txn,
                    &alice_pubkey,
                    ephemeral_scheduled_fn.clone(),
                    Some(ephemeral_future_schedule.clone()),
                    now,
                )
                .unwrap();
                assert_eq!(
                    vec![(
                        persisted_scheduled_fn.clone(),
                        Some(persisted_schedule.clone())
                    )],
                    live_scheduled_fns(txn, the_future, &alice_pubkey,).unwrap(),
                );
                assert_eq!(
                    vec![
                        (persisted_scheduled_fn, Some(persisted_schedule)),
                        (ephemeral_scheduled_fn, Some(ephemeral_future_schedule)),
                    ],
                    live_scheduled_fns(txn, the_distant_future, &alice_pubkey,).unwrap(),
                );

                Result::<(), DatabaseError>::Ok(())
            })
            .await
            .unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    // #[ignore = "flakey. Sometimes fails the last assert with 3 instead of 5"]
    #[cfg(feature = "test_utils")]
    async fn schedule_test() -> anyhow::Result<()> {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            ..
        } = RibosomeTestFixture::new(TestWasm::Schedule).await;

        // Let's just drive alice to exhaust all ticks.
        // let _schedule: () = conductor.call(&alice, "schedule", ()).await;
        let mut i: usize = 0;
        while i < 10 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            conductor.handle().dispatch_scheduled_fns().await;
            i = i + 1;
        }
        let query_tick: Vec<Record> = conductor.call(&alice, "query_tick", ()).await;
        assert_eq!(query_tick.len(), 5);

        // The persistent schedule should run once in second.
        let query_tock: Vec<Record> = conductor.call(&alice, "query_tock", ()).await;
        assert!(query_tock.len() < 3);

        // If Bob does a few ticks and then calls `start_scheduler` the
        // ephemeral scheduled task will be flushed so the ticks will not be
        // exhaused until the function is rescheduled.
        let _shedule: () = conductor.call(&bob, "schedule", ()).await;
        conductor.handle().dispatch_scheduled_fns().await;
        let query1: Vec<Record> = conductor.call(&bob, "query_tick", ()).await;
        assert_eq!(query1.len(), 1);
        conductor.handle().dispatch_scheduled_fns().await;
        let query2: Vec<Record> = conductor.call(&bob, "query_tick", ()).await;
        assert_eq!(query2.len(), query1.len() + 1);

        // With a fast scheduler bob should clear everything out.
        let _ = conductor
            .clone()
            .start_scheduler(std::time::Duration::from_millis(1))
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let q: Vec<Record> = conductor.call(&bob, "query_tick", ()).await;
        assert_eq!(q.len(), query2.len());

        // Rescheduling will allow bob catch up to alice.
        let _shedule: () = conductor.call(&bob, "schedule", ()).await;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let q2: Vec<Record> = conductor.call(&bob, "query_tick", ()).await;
        assert_eq!(q2.len(), 5);

        Ok(())
    }
}
