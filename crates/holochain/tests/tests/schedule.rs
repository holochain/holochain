use hdk::prelude::{BoxApi, ExternIO, InlineZomeResult};
use holochain::sweettest::{
    SweetCell, SweetConductor, SweetConductorConfig, SweetDnaFile, SweetLocalRendezvous,
};
use holochain::test_utils::{host_fn_caller::HostFnCaller, RibosomeTestFixture};
use holochain_sqlite::error::DatabaseError;
use holochain_state::mutations::schedule_fn;
use holochain_state::prelude::*;
use holochain_state::schedule::fn_is_scheduled;
use holochain_state::schedule::live_scheduled_fns;
use holochain_timestamp::Timestamp;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::signal::Signal;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::{
    Schedule::{self, Ephemeral, Persisted},
    ScheduledFn,
};
use holochain_zome_types::signal::AppSignal;
use tokio::sync::broadcast;
use tokio::time::error::Elapsed;

const SCHEDULED_FN: &str = "scheduled";
const SCHEDULING_FN: &str = "start";
const COORDINATOR: &str = "coordinator";

/// Test schedule ephemeral fn
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_ephemeral_ok() {
    holochain_trace::test_run();

    // Start with a duration of 3ms and decrease by 1ms each time it's called.
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        let ms: u64 = match input {
            None => 3,
            Some(Schedule::Ephemeral(duration)) => duration.as_millis() as u64,
            _ => panic!("Expected Ephemeral Schedule"),
        };
        match ms {
            0 => Ok(None),
            _ => Ok(Some(Schedule::Ephemeral(std::time::Duration::from_millis(
                ms - 1,
            )))),
        }
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    // Start test: schedule function
    conductor
        .call::<(), ()>(&cell.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;
    // Scheduled function is first called with None input.
    // Then should be called with decreasing durations until it gets unscheduled.
    assert_eq!(None, wait_for_signal(&mut app_signal).await.unwrap());
    assert_eq!(
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(2))),
        wait_for_signal(&mut app_signal).await.unwrap()
    );
    assert_eq!(
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(1))),
        wait_for_signal(&mut app_signal).await.unwrap()
    );
    assert_eq!(
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(0))),
        wait_for_signal(&mut app_signal).await.unwrap()
    );
    assert!(wait_for_signal(&mut app_signal).await.is_err());
    assert!(!is_scheduled(&host_fn_caller, &cell).await);
}

/// Test schedule ephemeral function which gives an error
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_ephemeral_error() {
    holochain_trace::test_run();

    // Start with a crontab that triggers every second, then have it return an error.
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        match input {
            None => Ok(Some(Ephemeral(std::time::Duration::from_secs(1)))),
            _ => Err(holochain::prelude::InlineZomeError::TestError(
                "Intentional error".to_string(),
            )),
        }
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    // Start test: schedule function
    conductor
        .call::<(), ()>(&cell.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;

    // Scheduled function is first called with None input.
    assert_eq!(None, wait_for_signal(&mut app_signal).await.unwrap());
    // Scheduled function is called with input from previous output.
    assert!(wait_for_signal(&mut app_signal).await.unwrap().is_some());
    // Should be unscheduled
    assert!(wait_for_signal(&mut app_signal).await.is_err());
    assert!(!is_scheduled(&host_fn_caller, &cell).await);
}

/// Test schedule a persisted function that changes the crontab a couple of times
/// before unscheduling.
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_persisted_fn_then_unschedule() {
    holochain_trace::test_run();

    // Start with a crontab that triggers every 3 secs, then decrease frequency by 1 sec
    // each time it's called until zero is reached.
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        let cron = match input {
            None => "*/3 * * * * * *".to_string(),
            Some(Schedule::Persisted(str)) => str,
            _ => panic!("Expected Persisted Schedule"),
        };
        let n: usize = cron.chars().nth(2).unwrap().to_digit(10).unwrap() as usize;
        let res = match n {
            1 => Ok(None),
            2 => Ok(Some(Schedule::Persisted("*/1 * * * * * *".to_string()))),
            3 => Ok(Some(Schedule::Persisted("*/2 * * * * * *".to_string()))),
            _ => panic!("Expected n < 4"),
        };
        res
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    // Start test: schedule function
    conductor
        .call::<(), ()>(&cell.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;

    // Scheduled function is first called with None input.
    assert_eq!(None, wait_for_signal(&mut app_signal).await.unwrap());
    // Scheduled function is called with input from previous output.
    assert_eq!(
        Some(Schedule::Persisted("*/2 * * * * * *".to_string())),
        wait_for_signal(&mut app_signal).await.unwrap()
    );
    // Scheduled function is called with input from previous output.
    assert_eq!(
        Some(Schedule::Persisted("*/1 * * * * * *".to_string())),
        wait_for_signal(&mut app_signal).await.unwrap()
    );
    // Should be unscheduled
    assert!(wait_for_signal(&mut app_signal).await.is_err());
    assert!(!is_scheduled(&host_fn_caller, &cell).await);
}

/// Test schedule the same persisted function in two different cells using the same agent pub key.
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_same_agent() {
    holochain_trace::test_run();
    // Set schedule to always trigger every 3 secs
    let zome = create_schedule_zome(|_api, input| {
        Ok(Some(input.unwrap_or(Schedule::Persisted(
            "*/3 * * * * * *".to_string(),
        ))))
    });
    let dna_0 = SweetDnaFile::unique_from_inline_zomes(zome.clone()).await;
    let dna_1 = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    // Set up 1 app with two different dnas but same agent pub key
    let app = conductor
        .setup_app("app", &[dna_0.0.clone(), dna_1.0.clone()])
        .await
        .unwrap();
    let cells = app.into_cells();
    let cell_0 = cells[0].clone();
    let pubkey_0 = cell_0.agent_pubkey().clone();
    let host_fn_caller_0 =
        HostFnCaller::create_for_zome(cell_0.cell_id(), &conductor.raw_handle(), &dna_0.0, 0).await;
    let cell_1 = cells[1].clone();
    let pubkey_1 = cell_1.agent_pubkey().clone();
    let host_fn_caller_1 =
        HostFnCaller::create_for_zome(cell_1.cell_id(), &conductor.raw_handle(), &dna_1.0, 0).await;

    assert_eq!(pubkey_0, pubkey_1);
    assert_ne!(cell_0.dna_hash(), cell_1.dna_hash());

    // Start test: schedule first cell
    conductor
        .call::<(), ()>(&cell_0.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;
    assert!(is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(!is_scheduled(&host_fn_caller_1, &cell_1).await);

    // schedule second cell
    conductor
        .call::<(), ()>(&cell_1.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;
    assert!(is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(is_scheduled(&host_fn_caller_1, &cell_1).await);

    // Unschedule first one
    host_fn_caller_0
        .authored_db
        .write_async(move |txn| {
            unschedule_fn(
                txn,
                &pubkey_0,
                &ScheduledFn::new(COORDINATOR.into(), SCHEDULED_FN.into()),
            );
            Result::<(), DatabaseError>::Ok(())
        })
        .await
        .unwrap();
    assert!(!is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(is_scheduled(&host_fn_caller_1, &cell_1).await);

    // Unschedule second one
    host_fn_caller_1
        .authored_db
        .write_async(move |txn| {
            unschedule_fn(
                txn,
                &pubkey_1,
                &ScheduledFn::new(COORDINATOR.into(), SCHEDULED_FN.into()),
            );
            Result::<(), DatabaseError>::Ok(())
        })
        .await
        .unwrap();
    assert!(!is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(!is_scheduled(&host_fn_caller_1, &cell_1).await);
}

/// Test schedule the same persisted function in two different cells using the same dna.
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_same_dna() {
    holochain_trace::test_run();
    // Set schedule to always trigger every 3 secs
    let zome = create_schedule_zome(|_api, input| {
        Ok(Some(input.unwrap_or(Schedule::Persisted(
            "*/3 * * * * * *".to_string(),
        ))))
    });
    let dna_0 = SweetDnaFile::unique_from_inline_zomes(zome.clone()).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    // Set up 2 apps each using the same dna but with different agent pub key
    let app0 = conductor
        .setup_app("app0", std::slice::from_ref(&dna_0.0))
        .await
        .unwrap();
    let app1 = conductor
        .setup_app("app1", std::slice::from_ref(&dna_0.0))
        .await
        .unwrap();
    let cell_0 = app0.into_cells()[0].clone();
    let dna_hash_0 = cell_0.dna_hash().clone();
    let pubkey_0 = cell_0.agent_pubkey().clone();
    let host_fn_caller_0 =
        HostFnCaller::create_for_zome(cell_0.cell_id(), &conductor.raw_handle(), &dna_0.0, 0).await;
    let cell_1 = app1.into_cells()[0].clone();
    let dna_hash_1 = cell_1.dna_hash().clone();
    let pubkey_1 = cell_1.agent_pubkey().clone();
    let host_fn_caller_1 =
        HostFnCaller::create_for_zome(cell_1.cell_id(), &conductor.raw_handle(), &dna_0.0, 0).await;

    assert_ne!(pubkey_0, pubkey_1);
    assert_eq!(dna_hash_0, dna_hash_1);

    // Start test: schedule first cell
    conductor
        .call::<(), ()>(&cell_0.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;
    assert!(is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(!is_scheduled(&host_fn_caller_1, &cell_1).await);

    // schedule second cell
    conductor
        .call::<(), ()>(&cell_1.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;
    assert!(is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(is_scheduled(&host_fn_caller_1, &cell_1).await);

    // Unschedule first one
    host_fn_caller_0
        .authored_db
        .write_async(move |txn| {
            unschedule_fn(
                txn,
                &pubkey_0,
                &ScheduledFn::new(COORDINATOR.into(), SCHEDULED_FN.into()),
            );
            Result::<(), DatabaseError>::Ok(())
        })
        .await
        .unwrap();
    assert!(!is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(is_scheduled(&host_fn_caller_1, &cell_1).await);

    // Unschedule second one
    host_fn_caller_1
        .authored_db
        .write_async(move |txn| {
            unschedule_fn(
                txn,
                &pubkey_1,
                &ScheduledFn::new(COORDINATOR.into(), SCHEDULED_FN.into()),
            );
            Result::<(), DatabaseError>::Ok(())
        })
        .await
        .unwrap();
    assert!(!is_scheduled(&host_fn_caller_0, &cell_0).await);
    assert!(!is_scheduled(&host_fn_caller_1, &cell_1).await);
}

/// Test persisted schedule with invalid crontab output which should unschedule the function.
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_persisted_fn_with_bad_crontab() {
    holochain_trace::test_run();

    // Start with a valid crontab then set an invalid one.
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        match input {
            None => Ok(Some(Schedule::Persisted("*/1 * * * * * *".to_string()))), // every second
            Some(_) => Ok(Some(Schedule::Persisted("*/0 * * * * * *".to_string()))), // invalid crontab
        }
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    conductor
        .call::<(), ()>(&cell.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;

    // Scheduled function is first called with None input
    assert_eq!(None, wait_for_signal(&mut app_signal).await.unwrap());

    // Scheduled function is then called with Some input
    assert!(wait_for_signal(&mut app_signal).await.unwrap().is_some());

    // On bad crontab scheduled function should unschedule
    assert!(wait_for_signal(&mut app_signal).await.is_err());
    assert!(!is_scheduled(&host_fn_caller, &cell).await);
}

/// Test schedule persisted function which gives an error, which should unschedule the function.
/// Assuming a scheduler interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_persisted_fn_that_errors() {
    holochain_trace::test_run();

    // Start with a crontab that triggers every second, then have it return an error.
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        match input {
            None => Ok(Some(Persisted("*/1 * * * * * *".to_string()))),
            _ => Err(holochain::prelude::InlineZomeError::TestError(
                "Intentional error".to_string(),
            )),
        }
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    // Start test: schedule function
    conductor
        .call::<(), ()>(&cell.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;

    // Scheduled function is first called with None input.
    assert_eq!(None, wait_for_signal(&mut app_signal).await.unwrap());
    // Scheduled function is called with input from previous output.
    assert_eq!(
        Some(Schedule::Persisted("*/1 * * * * * *".to_string())),
        wait_for_signal(&mut app_signal).await.unwrap()
    );
    // Should be unscheduled
    assert!(wait_for_signal(&mut app_signal).await.is_err());
    assert!(!is_scheduled(&host_fn_caller, &cell).await);
}

/// Test schedule persisted fn with no next crontab schedule
/// Assuming a schedular interval of 100ms
#[tokio::test(flavor = "multi_thread")]
async fn schedule_persisted_crontab_end() {
    holochain_trace::test_run();

    // Start with an outdated crontab that has no next schedule.
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        match input {
            None => Ok(Some(Persisted("* * * * * * 1984".to_string()))),
            _ => Ok(input),
        }
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    // Start test: schedule function
    conductor
        .call::<(), ()>(&cell.zome(COORDINATOR), SCHEDULING_FN, ())
        .await;
    // Should be scheduled
    assert!(is_scheduled(&host_fn_caller, &cell).await);
    // Scheduled function is first called with None input.
    assert_eq!(None, wait_for_signal(&mut app_signal).await.unwrap());
    // Should be unscheduled
    assert!(!is_scheduled(&host_fn_caller, &cell).await);
}

/// Test persisted fn with an expired crontab schedule
#[tokio::test(flavor = "multi_thread")]
async fn schedule_persisted_expired() {
    holochain_trace::test_run();

    // Scheduled function should not be called in this test
    let zome = create_schedule_zome(|api, input| {
        let _ = api.emit_signal(AppSignal::new(ExternIO::encode(input.clone()).unwrap()));
        Ok(input)
    });

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;
    let mut conductor = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        SweetLocalRendezvous::new().await,
    )
    .await;
    let app = conductor
        .setup_app("app", std::slice::from_ref(&dna.0))
        .await
        .unwrap();
    let cell = app.into_cells()[0].clone();
    let pubkey = cell.agent_pubkey().clone();
    let mut app_signal = conductor.subscribe_to_app_signals("app".into());
    let host_fn_caller =
        HostFnCaller::create_for_zome(cell.cell_id(), &conductor.raw_handle(), &dna.0, 0).await;

    // Should not be scheduled
    assert!(!is_scheduled(&host_fn_caller, &cell).await);

    // Schedule function with expired start time
    host_fn_caller
        .authored_db
        .write_async(move |txn| {
            let now = Timestamp::now();
            let expired = (now - std::time::Duration::from_secs(120)).unwrap();
            schedule_fn(
                txn,
                &pubkey,
                ScheduledFn::new(COORDINATOR.into(), SCHEDULED_FN.into()),
                Some(Schedule::Persisted("0 * * * * * *".into())), // every minute
                expired,
            )
            .unwrap();
            Result::<(), DatabaseError>::Ok(())
        })
        .await
        .unwrap();

    // Should be scheduled but expired
    assert!(is_scheduled(&host_fn_caller, &cell).await);
    assert!(wait_for_signal(&mut app_signal).await.is_err());
    assert!(is_scheduled(&host_fn_caller, &cell).await);
}

/// Helper for creating a zome with just one schedulable function called [`SCHEDULED_FN`]
/// that can be scheduled by calling [`SCHEDULING_FN`]
fn create_schedule_zome(
    func: impl Fn(BoxApi, Option<Schedule>) -> InlineZomeResult<Option<Schedule>>
        + 'static
        + Send
        + Sync,
) -> InlineZomeSet {
    InlineZomeSet::new_unique_single("integrity", COORDINATOR, vec![], 0)
        .function::<_, (), ()>(COORDINATOR, SCHEDULING_FN, |api, _| {
            let _ = api.schedule(SCHEDULED_FN.to_string());
            Ok(())
        })
        .function::<_, Option<Schedule>, Option<Schedule>>(COORDINATOR, SCHEDULED_FN, func)
}

/// Helper for checking if [`SCHEDULED_FN`] function has been scheduled
async fn is_scheduled(host_fn_caller: &HostFnCaller, cell: &SweetCell) -> bool {
    let pubkey = cell.cell_id().agent_pubkey().clone();
    host_fn_caller
        .authored_db
        .read_async({
            move |txn| {
                Result::<bool, DatabaseError>::Ok(
                    fn_is_scheduled(
                        txn,
                        ScheduledFn::new(COORDINATOR.into(), SCHEDULED_FN.into()),
                        &pubkey,
                    )
                    .unwrap(),
                )
            }
        })
        .await
        .unwrap()
}

/// Helper for waiting for next signal from cell
async fn wait_for_signal(
    app_signal: &mut broadcast::Receiver<Signal>,
) -> Result<Option<Schedule>, Elapsed> {
    let msg = tokio::time::timeout(std::time::Duration::from_secs(3), app_signal.recv())
        .await?
        .unwrap();
    match msg {
        Signal::App { signal, .. } => {
            let input: Option<Schedule> = signal.into_inner().decode().unwrap();
            Ok(input)
        }
        _ => panic!("Expected AppSignal, got {msg:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn schedule_test_low_level() -> anyhow::Result<()> {
    holochain_trace::test_run();
    let RibosomeTestFixture {
        alice_pubkey,
        alice_host_fn_caller,
        ..
    } = RibosomeTestFixture::new(TestWasm::Schedule).await;

    alice_host_fn_caller
        .authored_db
        .write_async(move |txn| {
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

            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey).unwrap());
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey).unwrap());

            // Deleting live ephemeral scheduled fns from now should delete.
            delete_live_ephemeral_scheduled_fns(txn, now, &alice_pubkey).unwrap();
            assert!(!fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap());

            schedule_fn(
                txn,
                &alice_pubkey,
                ephemeral_scheduled_fn.clone(),
                None,
                now,
            )
            .unwrap();
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap());

            // Deleting live ephemeral fns from a past time should do nothing.
            delete_live_ephemeral_scheduled_fns(txn, the_past, &alice_pubkey).unwrap();
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap());

            // Deleting live ephemeral fns from the future should delete.
            delete_live_ephemeral_scheduled_fns(txn, the_future, &alice_pubkey).unwrap();
            assert!(!fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap());

            // Deleting all ephemeral fns should delete.
            schedule_fn(
                txn,
                &alice_pubkey,
                ephemeral_scheduled_fn.clone(),
                None,
                now,
            )
            .unwrap();
            assert!(fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap());
            delete_all_ephemeral_scheduled_fns(txn).unwrap();
            assert!(!fn_is_scheduled(txn, ephemeral_scheduled_fn.clone(), &alice_pubkey,).unwrap());
            assert!(fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey,).unwrap());

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
                    Some(persisted_schedule.clone()),
                    false,
                )],
                live_scheduled_fns(txn, the_future, &alice_pubkey,).unwrap(),
            );
            assert_eq!(
                vec![
                    (persisted_scheduled_fn, Some(persisted_schedule), false),
                    (
                        ephemeral_scheduled_fn,
                        Some(ephemeral_future_schedule),
                        true
                    ),
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
    holochain_trace::test_run();
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
    conductor
        .start_scheduler(std::time::Duration::from_millis(1_000_000_000))
        .await?;

    // At first nothing has happened because init won't run until some zome
    // call runs.
    let query_tick: Vec<Record> = conductor.call(&alice, "query_tick_init", ()).await;
    assert!(query_tick.is_empty());

    // Wait to make sure we've init, but it should have happened for sure.
    while {
        !alice_host_fn_caller
            .authored_db
            .write_async({
                let alice_pubkey = alice_pubkey.clone();
                move |txn| {
                    let persisted_scheduled_fn = ScheduledFn::new(
                        TestWasm::Schedule.into(),
                        "cron_scheduled_fn_init".into(),
                    );

                    Result::<bool, DatabaseError>::Ok(
                        fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &alice_pubkey)
                            .unwrap(),
                    )
                }
            })
            .await
            .unwrap()
    } {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }

    // Round up to the next second so we don't trigger two tocks in quick
    // succession.
    let mut now =
        Timestamp::from_micros((Timestamp::now().as_micros() / 1_000_000 + 1) * 1_000_000 + 1);

    // The ephemeral function will dispatch each millisecond.
    // The tock will dispatch once and wait a second.
    let mut i: usize = 0;
    while i < 10 {
        conductor.dispatch_scheduled_fns(now).await;
        now = (now + std::time::Duration::from_millis(2))?;
        i += 1;
    }
    loop {
        let query_tick_init: Vec<Record> = conductor.call(&alice, "query_tick_init", ()).await;
        let query_tock_init: Vec<Record> = conductor.call(&alice, "query_tock_init", ()).await;
        if query_tick_init.len() == 5 && query_tock_init.len() == 1 {
            break;
        }
    }

    // after a second the tock will run again.
    now = (now + std::time::Duration::from_millis(1000))?;
    conductor.dispatch_scheduled_fns(now).await;
    loop {
        let query_tick_init: Vec<Record> = conductor.call(&alice, "query_tick_init", ()).await;
        let query_tock_init: Vec<Record> = conductor.call(&alice, "query_tock_init", ()).await;
        if query_tick_init.len() == 5 && query_tock_init.len() == 2 {
            break;
        }
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
        conductor.dispatch_scheduled_fns(now).await;
        now = (now + std::time::Duration::from_millis(2))?;
        i += 1;
    }
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let query_tick: Vec<Record> = conductor.call(&alice, "query_tick", ()).await;
        let query_tock: Vec<Record> = conductor.call(&alice, "query_tock", ()).await;
        if query_tick.len() == 5 && query_tock.len() == 1 {
            break;
        }
    }

    // after a second the tock will run again.
    now = (now + std::time::Duration::from_millis(1000))?;
    conductor.dispatch_scheduled_fns(now).await;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let query_tick: Vec<Record> = conductor.call(&alice, "query_tick", ()).await;
        let query_tock: Vec<Record> = conductor.call(&alice, "query_tock", ()).await;
        if query_tick.len() == 5 && query_tock.len() == 2 {
            break;
        }
    }

    // Starting the scheduler should flush ephemeral.
    let _schedule: () = conductor.call(&bob, "schedule", ()).await;

    assert!(bob_host_fn_caller
        .authored_db
        .write_async({
            let bob_pubkey = bob_pubkey.clone();
            move |txn| {
                let persisted_scheduled_fn =
                    ScheduledFn::new(TestWasm::Schedule.into(), "scheduled_fn".into());

                Result::<bool, DatabaseError>::Ok(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &bob_pubkey).unwrap(),
                )
            }
        })
        .await
        .unwrap());

    conductor
        .start_scheduler(std::time::Duration::from_millis(1_000_000_000))
        .await?;

    assert!(!bob_host_fn_caller
        .authored_db
        .write_async({
            let bob_pubkey = bob_pubkey.clone();
            move |txn| {
                let persisted_scheduled_fn =
                    ScheduledFn::new(TestWasm::Schedule.into(), "scheduled_fn".into());

                Result::<bool, DatabaseError>::Ok(
                    fn_is_scheduled(txn, persisted_scheduled_fn.clone(), &bob_pubkey).unwrap(),
                )
            }
        })
        .await
        .unwrap());

    Ok(())
}
