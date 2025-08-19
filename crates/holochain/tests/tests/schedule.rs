use hdk::prelude::{BoxApi, ExternIO, InlineZomeResult};
use holochain::sweettest::{SweetCell, SweetConductor, SweetDnaFile};
use holochain::test_utils::host_fn_caller::HostFnCaller;
use holochain_sqlite::error::DatabaseError;
use holochain_state::mutations::schedule_fn;
use holochain_state::schedule::fn_is_scheduled;
use holochain_timestamp::Timestamp;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::signal::Signal;
use holochain_zome_types::prelude::Schedule::{Ephemeral, Persisted};
use holochain_zome_types::prelude::{Schedule, ScheduledFn};
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
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor.setup_app("app", &[dna.0.clone()]).await.unwrap();
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
        _ => panic!("Expected AppSignal, got {:?}", msg),
    }
}
