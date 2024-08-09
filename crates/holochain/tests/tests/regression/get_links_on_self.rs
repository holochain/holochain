use hdi::prelude::*;
use hdk::prelude::*;
use holochain::sweettest::*;

fn validate_create(
    h: Box<dyn HostFnApiT>,
    author: AgentPubKey,
    action_hash: ActionHash,
) -> Result<ValidateCallbackResult, HostFnApiError> {
    let activity = h.must_get_agent_activity(MustGetAgentActivityInput {
        author,
        chain_filter: ChainFilter::new(action_hash),
    })?;
    let rs: Vec<_> = activity
        .iter()
        .filter_map(|a| {
            h.must_get_valid_record(MustGetValidRecordInput(a.action.action_address().clone()))
                .ok()
        })
        .collect();
    Ok(ValidateCallbackResult::Valid)
}

#[tokio::test(flavor = "multi_thread")]
async fn get_links_on_self() {
    holochain_trace::test_run();

    const N: usize = 20;

    let config = SweetConductorConfig::rendezvous(true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(N, config).await;

    let entry_def = EntryDef::default_from_id("entry_def_id");
    let zomes = SweetInlineZomes::new(vec![entry_def.clone()], 0)
        .function("create_item", |h, base: AgentPubKey| {
            let location = EntryDefLocation::app(0, 0);
            let visibility = EntryVisibility::Public;
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let addr = h.create(CreateInput::new(
                location.clone(),
                visibility,
                entry,
                ChainTopOrdering::default(),
            ))?;
            h.create_link(CreateLinkInput {
                base_address: base.into(),
                target_address: addr.into(),
                zome_index: 0.into(),
                link_type: 0.into(),
                tag: LinkTag::new(vec![]),
                chain_top_ordering: ChainTopOrdering::default(),
            })?;
            Ok(())
        })
        .function("get_links", |h, base: AgentPubKey| {
            let mut links = h.get_links(vec![GetLinksInput {
                base_address: base.into(),
                link_type: LinkTypeFilter::single_dep(0.into()),
                get_options: GetOptions::default(),
                tag_prefix: None,
                after: None,
                before: None,
                author: None,
            }])?;
            Ok(links.pop().unwrap())
        })
        .integrity_function("validate", |h, op: Op| match op {
            Op::StoreEntry(e) => Ok(validate_create(
                h,
                e.action.hashed.author().clone(),
                e.action.to_hash(),
            )?),
            Op::StoreRecord(e) => Ok(validate_create(
                h,
                e.record.action().author().clone(),
                e.record.action().to_hash(),
            )?),
            _ => Ok(ValidateCallbackResult::Valid),
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    let cells = conductors
        .setup_app("app", &[dna_file])
        .await
        .unwrap()
        .cells_flattened();
    let bobkey = cells[1].agent_pubkey().clone();

    let _: () = conductors[0]
        .call_fallible(&cells[0].zome("coordinator"), "create_item", bobkey.clone())
        .await
        .unwrap();

    let mut done: HashSet<usize> = (0..conductors.len()).collect();
    let mut times = vec![0; N];
    let start = std::time::Instant::now();

    while !done.is_empty() {
        for i in done.clone() {
            let links: Vec<Link> = conductors[i]
                .call_fallible(&cells[i].zome("coordinator"), "get_links", bobkey.clone())
                .await
                .unwrap();
            if !links.is_empty() {
                done.remove(&i);
                times[i] = start.elapsed().as_millis();
            }
        }
        if !done.is_empty() {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
    }

    println!("Time to complete for each node:\n{:?}", times);
}

#[hdk_entry_helper]
pub struct A;

/// Entry type enum for hc demo-cli.
#[hdk_entry_types(skip_hdk_extern = true)]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    ET(A),
}

/// Link type enum for hc demo-cli.
#[hdk_link_types]
pub enum LinkTypes {
    LT,
}
