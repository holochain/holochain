use hdk::prelude::*;

const TICKS: usize = 5;

#[hdk_entry(id = "tick")]
struct Tick;

entry_defs![Tick::entry_def()];

#[hdk_extern(infallible)]
fn scheduled_fn(_: Option<Schedule>) -> Option<Schedule> {
    if create_entry(&Tick).is_err() {
        return Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)));
    }
    if query(ChainQueryFilter::default().entry_type(entry_type!(Tick).unwrap())).unwrap().len() < TICKS {
        Some(Schedule::Ephemeral(std::time::Duration::from_millis(1)))
    }
    else {
        None
    }
}

#[hdk_extern]
fn schedule(_: ()) -> ExternResult<()> {
    hdk::prelude::schedule("scheduled_fn")
}