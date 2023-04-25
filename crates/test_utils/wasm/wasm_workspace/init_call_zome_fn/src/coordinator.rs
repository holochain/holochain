use hdi::prelude::holo_hash::hash_type::AnyLinkable;
use hdk::hash_path::path::Component;
use hdk::prelude::*;
use crate::integrity::{ANCHOR, EntryTypes, LinkTypes, Test};

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    today_path()?.typed(LinkTypes::MyLink)?.ensure()?;

    Ok(InitCallbackResult::Pass)
}

fn today_path() -> ExternResult<Path> {
    let mut root_path = Path::from(ANCHOR);
    root_path.append_component(Component::from(format!("{}", today()?)));
    Ok(root_path)
}

fn today() -> ExternResult<i64> {
    Ok(sys_time()?.as_seconds_and_nanos().0 / (60 * 60 * 24))
}

fn today_hours_path() -> ExternResult<Path> {
    let mut root_path = Path::from(ANCHOR);
    root_path.append_component(Component::from(format!("{}", today()?)));
    root_path.append_component(Component::from(format!("{}", today_hours()?)));
    Ok(root_path)
}

fn today_hours() -> ExternResult<i64> {
    Ok(sys_time()?.as_seconds_and_nanos().0 / (60 * 60) - today()?)
}

#[hdk_extern]
fn create_test(test: Test) -> ExternResult<ActionHash> {
    let path = today_hours_path()?;
    path.clone().typed(LinkTypes::MyLink)?.ensure()?;

    let entry_hash = hash_entry(test.clone())?;
    let action_hash = create_entry(&EntryTypes::Test(test))?;

    hdk::prelude::create_link(path.path_entry_hash()?, entry_hash, LinkTypes::MyLink, ())?;

    Ok(action_hash)
}

#[derive(Deserialize, Debug)]
pub struct CreateLinkPayload {
    from: AnyLinkableHash,
    to: AnyLinkableHash,
}

#[hdk_extern]
fn create_link(payload: CreateLinkPayload) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(payload.from, payload.to, LinkTypes::MyLink, ())
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Option<Record>>> {
    let mut today_path = today_hours_path()?;
    let today_path = today_path.typed(LinkTypes::MyLink)?;
    let today_path_entry_hash = today_path.path_entry_hash()?;

    let links = hdk::prelude::get_links(today_path_entry_hash, LinkTypes::MyLink, None)?;

    let mut records = vec![];
    for link in links {
        let record = match link.target.hash_type() {
            AnyLinkable::Action => {
                get::<AnyDhtHash>(link.target.clone().into_action_hash().unwrap().into(), GetOptions::content())?
            },
            AnyLinkable::Entry => {
                get::<AnyDhtHash>(link.target.clone().into_entry_hash().unwrap().into(), GetOptions::content())?
            },
            _ => {
                None
            }
        };
        records.push(record);
    }

    Ok(records)
}

#[hdk_extern]
fn get_many(payload: Vec<ActionHash>) -> ExternResult<Vec<Option<Details>>> {
    let gets: Vec<Option<Details>> = payload.iter().map(|h| get_details(h.clone(), GetOptions::content()).unwrap()).collect();

    Ok(gets)
}
