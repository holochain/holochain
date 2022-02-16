use futures::StreamExt;
use holochain_types::prelude::fake_dna_zomes_named;
use holochain_types::prelude::write_fake_dna_file;
use holochain_wasm_test_utils::TestWasm;

#[path = "../tests/test_utils/mod.rs"]
mod test_utils;

use test_utils::*;

#[tokio::main]
pub async fn main() {
    static NUM_DNA: u8 = 100;
    static NUM_CONCURRENT_INSTALLS: usize = 2;
    static REQ_TIMEOUT_MS: u64 = 30000;

    observability::test_run().ok();
    // NOTE: This is a full integration test that
    // actually runs the holochain binary

    let admin_port = 9211;

    let zomes = vec![(TestWasm::Foo.into(), TestWasm::Foo.into())];
    let (client, _) = websocket_client_by_port(admin_port).await.unwrap();

    let install_tasks_stream = futures::stream::iter((0..NUM_DNA).into_iter().map(|i| {
        let mut client = client.clone();
        let zomes = zomes.clone();

        tokio::spawn(async move {
            let agent_key = generate_agent_pubkey(&mut client, REQ_TIMEOUT_MS).await;
            println!("[{}] Agent pub key generated: {}", i, agent_key);

            // Install Dna
            let name = format!("fake_dna_{}", i);
            let dna = fake_dna_zomes_named(&uuid::Uuid::new_v4().to_string(), &name, zomes);

            let original_dna_hash = dna.dna_hash().clone();
            let (fake_dna_path, _tmpdir) = write_fake_dna_file(dna.clone()).await.unwrap();
            let dna_hash = register_and_install_dna_named(
                &mut client,
                original_dna_hash.clone(),
                agent_key,
                fake_dna_path.clone(),
                None,
                name.clone(),
                name.clone(),
                REQ_TIMEOUT_MS,
            )
            .await;

            println!(
                "[{}] installed dna with hash {} and name {}",
                i, dna_hash, name
            );
        })
    }))
    .buffer_unordered(NUM_CONCURRENT_INSTALLS);

    let install_tasks = futures::StreamExt::collect::<Vec<_>>(install_tasks_stream);

    for r in install_tasks.await {
        r.unwrap();
    }
}
