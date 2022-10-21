use crate::integrity::*;
use hdk::prelude::*;

fn _scheduled_fn(entry_types_unit: EntryTypesUnit, entry: Entry) -> Option<Schedule> {
    if HDK
        .with(|h| {
            h.borrow().create(CreateInput::new(
                ScopedEntryDefIndex::try_from(entry_types_unit)?,
                EntryVisibility::Public,
                Tick.try_into().unwrap(),
                // This will be running concurrently with cron_scheduled_fn.
                ChainTopOrdering::Relaxed,
            ))
        })
        .is_err()
    {
        return Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)));
    }
    if hdk::prelude::query(
        ChainQueryFilter::default().entry_type(entry_types_unit.try_into().unwrap()),
    )
    .unwrap()
    .len()
        < TICKS
    {
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)))
    } else {
        None
    }
}

#[hdk_extern(infallible)]
fn scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    _scheduled_fn(EntryTypesUnit::Tick, Tick.try_into().unwrap())
}

#[hdk_extern(infallible)]
fn scheduled_fn_init(_: Option<Schedule>) -> Option<Schedule> {
    _scheduled_fn(EntryTypesUnit::TickInit, TickInit.try_into().unwrap())
}

fn _cron_scheduled_fn(entry_types_unit: EntryTypesUnit, entry: Entry) -> Option<Schedule> {
    HDK.with(|h| {
        h.borrow().create(CreateInput::new(
            ScopedEntryDefIndex::try_from(entry_types_unit)?,
            EntryVisibility::Public,
            entry,
            // This will be running concurrently with scheduled_fn.
            ChainTopOrdering::Relaxed,
        ))
    })
    .ok();
    Some(Schedule::Persisted("* * * * * * *".to_string()))
}

#[hdk_extern(infallible)]
fn cron_scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    _cron_scheduled_fn(EntryTypesUnit::Tock, Tock.try_into().unwrap())
}

#[hdk_extern(infallible)]
fn cron_scheduled_fn_init(_: Option<Schedule>) -> Option<Schedule> {
    _cron_scheduled_fn(EntryTypesUnit::TockInit, TockInit.try_into().unwrap())
}

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    hdk::prelude::schedule("scheduled_fn_init")?;
    hdk::prelude::schedule("cron_scheduled_fn_init")?;
    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn schedule(_: ()) -> ExternResult<()> {
    hdk::prelude::schedule("scheduled_fn")?;
    hdk::prelude::schedule("cron_scheduled_fn")?;
    Ok(())
}

fn _query(entry_types_unit: EntryTypesUnit) -> ExternResult<Vec<Record>> {
    hdk::prelude::query(
        ChainQueryFilter::default().entry_type(entry_types_unit.try_into().unwrap())
    )
}

#[hdk_extern]
fn query_tick(_: ()) -> ExternResult<Vec<Record>> {
    _query(EntryTypesUnit::Tick)
}

#[hdk_extern]
fn query_tick_init(_: ()) -> ExternResult<Vec<Record>> {
    _query(EntryTypesUnit::TickInit)
}

#[hdk_extern]
fn query_tock(_: ()) -> ExternResult<Vec<Record>> {
    _query(EntryTypesUnit::Tock)
}

#[hdk_extern]
fn query_tock_init(_: ()) -> ExternResult<Vec<Record>> {
    _query(EntryTypesUnit::TockInit)
}