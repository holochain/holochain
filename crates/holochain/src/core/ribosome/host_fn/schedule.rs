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
        _ => {
            Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "schedule".into(),
            )
            .to_string(),
        ))
        .into())
    },
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
                let persisted_schedule = Schedule::Persisted("* * * * * * *".into());

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
    #[cfg(feature = "test_utils")]
    async fn schedule_test_wasm() -> anyhow::Result<()> {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            alice_pubkey,
            alice_host_fn_caller,
            bob,
            bob_pubkey,
            bob_host_fn_caller,
            ..
        } = RibosomeTestFixture::new(TestWasm::Schedule).await;

        // We don't want the scheduler running and messing with our calculations.
        conductor.handle().start_scheduler(std::time::Duration::from_millis(1000_000_000)).await;

        // At first nothing has happened because init won't run until some zome
        // call runs.
        let query_tick: Vec<Record> = conductor.call(&alice, "query_tick_init", ()).await;
        assert!(query_tick.is_empty());

        // Wait to make sure we've init, but it should have happened for sure.
        while { let alice_pubkey = alice_pubkey.clone();
            !alice_host_fn_caller.authored_db.async_commit(move |txn: &mut Transaction| {
            let persisted_scheduled_fn = ScheduledFn::new(TestWasm::Schedule.into(), "cron_scheduled_fn_init".into());

            Result::<bool, DatabaseError>::Ok(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey).unwrap())
        }).await.unwrap() } {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }

        // Round up to the next second so we don't trigger two tocks in quick
        // succession.
        let mut now = Timestamp::from_micros((Timestamp::now().as_micros() / 1_000_000 + 1) * 1_000_000 + 1);

        // The ephemeral function will dispatch each millisecond.
        // The tock will dispatch once and wait a second.
        let mut i: usize = 0;
        while i < 10 {
            conductor.handle().dispatch_scheduled_fns(now).await;
            now = (now + std::time::Duration::from_millis(2))?;
            i = i + 1;
        }
        loop {
            let query_tick_init: Vec<Record> = conductor.call(&alice, "query_tick_init", ()).await;
            let query_tock_init: Vec<Record> = conductor.call(&alice, "query_tock_init", ()).await;
            if query_tick_init.len() == 5 && query_tock_init.len() == 1 { break; }
        }

        // after a second the tock will run again.
        now = (now + std::time::Duration::from_millis(1000))?;
        conductor.handle().dispatch_scheduled_fns(now).await;
        loop {
            let query_tick_init: Vec<Record> = conductor.call(&alice, "query_tick_init", ()).await;
            let query_tock_init: Vec<Record> = conductor.call(&alice, "query_tock_init", ()).await;
            if query_tick_init.len() == 5 && query_tock_init.len() == 2 { break; }
        }

        // alice can schedule things outside of init.
        let query_tock: Vec<Record> = conductor.call(&alice, "query_tock", ()).await;
        assert!(query_tock.is_empty());

        let _schedule: () = conductor.call(&alice, "schedule", ()).await;

        // Round up to the next second so we don't trigger two tocks in quick
        // succession.
        now = Timestamp::from_micros((Timestamp::now().as_micros() / 1_000_000 + 1) * 1_000_000 + 1);

        let mut i: usize = 0;
        while i < 10 {
            conductor.handle().dispatch_scheduled_fns(now).await;
            now = (now + std::time::Duration::from_millis(2))?;
            i = i + 1;
        }
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            let query_tick: Vec<Record> = conductor.call(&alice, "query_tick", ()).await;
            let query_tock: Vec<Record> = conductor.call(&alice, "query_tock", ()).await;
            if query_tick.len() == 5 && query_tock.len() == 1 { break; }
        }

        // after a second the tock will run again.
        now = (now + std::time::Duration::from_millis(1000))?;
        conductor.handle().dispatch_scheduled_fns(now).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            let query_tick: Vec<Record> = conductor.call(&alice, "query_tick", ()).await;
            let query_tock: Vec<Record> = conductor.call(&alice, "query_tock", ()).await;
            if query_tick.len() == 5 && query_tock.len() == 2 { break; }
        }

        // Starting the scheduler should flush ephemeral.
        let _schedule: () = conductor.call(&bob, "schedule", ()).await;

        assert!({ let bob_pubkey = bob_pubkey.clone();
            bob_host_fn_caller.authored_db.async_commit(move |txn: &mut Transaction| {
            let persisted_scheduled_fn = ScheduledFn::new(TestWasm::Schedule.into(), "scheduled_fn".into());

            Result::<bool, DatabaseError>::Ok(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &bob_pubkey).unwrap())
        }).await.unwrap() });

        conductor.handle().start_scheduler(std::time::Duration::from_millis(1000_000_000)).await;

        assert!(!{ let bob_pubkey = bob_pubkey.clone();
            bob_host_fn_caller.authored_db.async_commit(move |txn: &mut Transaction| {
            let persisted_scheduled_fn = ScheduledFn::new(TestWasm::Schedule.into(), "scheduled_fn".into());

            Result::<bool, DatabaseError>::Ok(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &bob_pubkey).unwrap())
        }).await.unwrap() });

        Ok(())
    }
}
