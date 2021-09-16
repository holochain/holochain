use hdk::prelude::*;

const TICKS: usize = 5;

#[hdk_entry(id = "tick")]
struct Tick;

#[hdk_entry(id = "tock")]
struct Tock;

entry_defs![Tick::entry_def(), Tock::entry_def()];

#[hdk_extern(infallible)]
fn scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    if create_entry(&Tick).is_err() {
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
    create_entry(&Tock).ok();
    Some(Schedule::Persisted("* * * * * * *".to_string()))
}

#[hdk_extern]
fn schedule(_: ()) -> ExternResult<()> {
    hdk::prelude::schedule("scheduled_fn")?;
    hdk::prelude::schedule("cron_scheduled_fn")?;
    Ok(())
}

#[hdk_extern]
fn query(_: ()) -> ExternResult<Vec<Element>> {
    hdk::prelude::query(ChainQueryFilter::default().entry_type(entry_type!(Tick).unwrap()))
}