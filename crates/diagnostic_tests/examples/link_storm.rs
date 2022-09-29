//! Create a new entry and add a link to it, repeatedly, as fast as possible.
//! The link's base is one of the agent's pubkeys, selected randomly.
//! Every once in a while, a random agent gets links on another agent's base,
//! and prints out how many they got vs how many are expected.
//! After a certain number of links, the link creation stops to let
//! gossip catch up. The test continues indefinitely.

use std::io::Write;
use std::time::{Duration, Instant};

use chashmap::CHashMap;
use colored::*;
use holochain_diagnostics::holochain::prelude::*;
use holochain_diagnostics::holochain::sweettest::{self, SweetConductorBatch, SweetDnaFile};
use holochain_diagnostics::*;

#[tokio::main]
async fn main() {
    let num_nodes = 120;
    let entry_size = 10_000;
    let max_links = 300;
    let loop_interval = Duration::from_millis(100);
    let get_interval = Duration::from_secs(5);

    let start = Instant::now();
    // let config = standard_config();
    let config = config_historical_and_agent_gossip_only();

    let mut conductors = SweetConductorBatch::from_config(num_nodes, config).await;
    println!("Conductors created (t={:3.1?}).", start.elapsed());

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(("zome", basic_zome())).await;
    let apps = conductors.setup_app("basic", &[dna]).await.unwrap();
    let cells = apps.cells_flattened();
    println!("Apps setup (t={:3.1?}).", start.elapsed());

    let mut rng = seeded_rng(None);

    let counts: CHashMap<_, _> = cells
        .iter()
        .map(|c| (c.agent_pubkey().clone(), 0))
        .collect();

    let content = |rng: &mut StdRng| {
        std::iter::repeat_with(|| rng.gen())
            .take(entry_size)
            .collect::<Vec<u8>>()
    };

    // TODO: write a "sparse" exchange of peer info, because 100x100 is too much.
    //       the fn can ensure that total connectedness is achieved. agent gossip can fill
    //       in the gaps.
    conductors.exchange_peer_info_sampled(&mut rng, 10).await;
    println!("Peer info exchanged (t={:3.1?}).", start.elapsed());

    drop(start);
    let start = Instant::now();

    let mut links = 0;
    let mut last_get = Instant::now();
    let mut print_last_notice = true;
    loop {
        let i = rng.gen_range(0..cells.len());
        let j = rng.gen_range(0..cells.len());

        // get links and print out actual vs expected count
        if last_get.elapsed() > get_interval {
            let agent = cells[j].agent_pubkey();
            let expected_count = *counts.get(agent).unwrap();
            let actual_count: usize = conductors[i]
                .call(&cells[i].zome("zome"), "link_count", agent.clone())
                .await;
            let inequality = if actual_count < expected_count {
                format!("{:>4} < {:<4}", actual_count, expected_count).red()
            } else if actual_count == expected_count {
                format!("{:>4} = {:<4}", actual_count, expected_count).green()
            } else {
                panic!("actual > expected");
            };
            println!();
            print!(
                "t={:6.1?} #={:>4} | {:>3} get {:<3} | {} ",
                start.elapsed(),
                links,
                i,
                j,
                inequality
            );
            std::io::stdout().flush().ok();
            last_get = Instant::now();
        }

        if print_last_notice && links == max_links {
            println!("\nNo more links will be created after this point.");
            print_last_notice = false;
        }

        // add a link for the first N steps
        if links < max_links {
            let base = cells[j].agent_pubkey();
            let mut count = counts.get_mut(base).unwrap();

            let _: ActionHash = conductors[i]
                .call(
                    &cells[i].zome("zome"),
                    "create",
                    (base.clone(), content(&mut rng)),
                )
                .await;
            *count += 1;
            links += 1;
            print!(".");
        }
        std::io::stdout().flush().ok();

        tokio::time::sleep(loop_interval).await;
    }
}

fn basic_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique([EntryDef::from_id("a")], 1)
        .function("create", |api, (agent, bytes): (AgentPubKey, Vec<u8>)| {
            let entry: SerializedBytes = UnsafeBytes::from(bytes).try_into().unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                Entry::App(AppEntryBytes(entry)),
                ChainTopOrdering::default(),
            ))?;
            let _ = api.create_link(CreateLinkInput::new(
                agent.into(),
                hash.clone().into(),
                ZomeId(0),
                LinkType::new(0),
                ().into(),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("link_count", |api, agent: AgentPubKey| {
            let links = api
                .get_links(vec![GetLinksInput::new(
                    agent.into(),
                    LinkTypeFilter::single_dep(0.into()),
                    None,
                )])
                .unwrap();
            Ok(links.first().unwrap().len())
        })
        .function("validate", |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        })
}
