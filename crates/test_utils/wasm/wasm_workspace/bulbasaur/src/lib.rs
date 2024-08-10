use hdk::prelude::*;

mod integrity;
use integrity::*;

#[hdk_extern]
fn create_item(base: AgentPubKey) -> ExternResult<()> {
    // let location = EntryDefLocation::app(0, 0);
    // let visibility = EntryVisibility::Public;
    // let entry = Entry::app(().try_into().unwrap()).unwrap();
    let addr: ActionHash = create_entry(EntryTypes::A(A))?;
    create_link(base, addr, LinkTypes::T, ())?;
    Ok(())
}

#[hdk_extern]
fn get_them_links(base: AgentPubKey) -> ExternResult<Vec<Link>> {
    let mut links = get_links(GetLinksInputBuilder::try_new(base, ..)?.build())?;
    Ok(links)
}
