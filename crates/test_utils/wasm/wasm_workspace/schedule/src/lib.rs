use hdk::prelude::*;

const TICKS: usize = 5;

#[hdk_entry(id = "tick")]
struct Tick;

#[hdk_entry(id = "tock")]
struct Tock;

entry_defs![Tick::entry_def(), Tock::entry_def()];

#[hdk_extern(infallible)]
fn scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    if HDK.with(|h| {
        h.borrow().create(CreateInput::new(
            Tick.into(),
            Tick.try_into().unwrap(),
            // This will be running concurrently with cron_scheduled_fn.
            ChainTopOrdering::Relaxed,
        ))
    }).is_err() {
        return Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)));
    }
    if hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type!(Tick).unwrap())).unwrap().len() < TICKS {
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)))
    }
    else {
        None
    }
}

#[hdk_extern(infallible)]
fn cron_scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    HDK.with(|h| {
        h.borrow().create(CreateInput::new(
            Tock.into(),
            Tock.try_into().unwrap(),
            // This will be running concurrently with scheduled_fn.
            ChainTopOrdering::Relaxed,
        ))
    }).ok();
    Some(Schedule::Persisted("* * * * * * *".to_string()))
}

#[hdk_extern]
fn schedule(_: ()) -> ExternResult<()> {
    hdk::prelude::schedule("scheduled_fn")?;
    hdk::prelude::schedule("cron_scheduled_fn")?;
    Ok(())
}

#[hdk_extern]
fn query_tick(_: ()) -> ExternResult<Vec<Element>> {
    hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type!(Tick).unwrap()))
}

#[hdk_extern]
fn query_tock(_: ()) -> ExternResult<Vec<Element>> {
    hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type!(Tock).unwrap()))
}
