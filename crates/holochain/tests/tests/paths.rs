use std::time::Duration;

use holochain::sweettest::{await_consistency, SweetConductorBatch, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
pub struct BookEntry {
    pub name: String,
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_can_find_entries_at_paths() {
    holochain_trace::test_run();

    let mut conductor_batch = SweetConductorBatch::from_standard_config_rendezvous(2).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Paths]).await;

    let apps = conductor_batch
        .setup_app("paths_app", [&dna])
        .await
        .unwrap();

    let ((alice_cell,), (bob_cell,)) = apps.into_tuples();

    conductor_batch[0]
        .declare_full_storage_arcs(dna.dna_hash())
        .await;
    conductor_batch[1]
        .declare_full_storage_arcs(dna.dna_hash())
        .await;

    // Wait for gossip to start
    conductor_batch[0]
        .require_initial_gossip_activity_for_cell(&alice_cell, 1, Duration::from_secs(30))
        .await
        .unwrap();

    // There should be no books created yet.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Shakespeare",
        )
        .await;
    assert!(books.is_empty());

    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Shakespeare",
        )
        .await;
    assert!(books.is_empty());

    // Alice adds a book entry.
    let () = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "add_book_entry",
            ("Shakespeare", "Romeo and Juliet"),
        )
        .await;

    // Alice should see her own entry.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Shakespeare",
        )
        .await;
    assert_eq!(
        books,
        [BookEntry {
            name: "Romeo and Juliet".to_string()
        }]
    );

    await_consistency(Duration::from_secs(60), [&alice_cell, &bob_cell])
        .await
        .unwrap();

    // After consistency, Bob should see Alice's entry.
    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Shakespeare",
        )
        .await;
    assert_eq!(
        books,
        [BookEntry {
            name: "Romeo and Juliet".to_string()
        }]
    );
}
