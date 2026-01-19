use holochain::sweettest::{await_consistency, SweetConductor, SweetConductorBatch, SweetDnaFile};
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
    conductor_batch.exchange_peer_info().await;

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

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

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

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

#[tokio::test(flavor = "multi_thread")]
async fn agents_can_find_multiple_entries_at_same_path() {
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
    conductor_batch.exchange_peer_info().await;

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

    // There should be no books created yet.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Stevenson",
        )
        .await;
    assert!(books.is_empty());

    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Stevenson",
        )
        .await;
    assert!(books.is_empty());

    // Alice adds a book entry.
    let () = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "add_book_entry",
            ("Stevenson", "Treasure Island"),
        )
        .await;

    // Bob adds a book entry.
    let () = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "add_book_entry",
            ("Stevenson", "Strange Case of Dr Jekyll and Mr Hyde"),
        )
        .await;

    // Alice should at least see her own entry.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Stevenson",
        )
        .await;
    assert!(books.contains(&BookEntry {
        name: "Treasure Island".to_string()
    }));

    // Bob should at least see his own entry.
    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Stevenson",
        )
        .await;
    assert!(books.contains(&BookEntry {
        name: "Strange Case of Dr Jekyll and Mr Hyde".to_string()
    }));

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

    // After consistency, both should see each other's entries.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Stevenson",
        )
        .await;
    assert_eq!(
        books,
        [
            BookEntry {
                name: "Treasure Island".to_string()
            },
            BookEntry {
                name: "Strange Case of Dr Jekyll and Mr Hyde".to_string()
            }
        ]
    );
    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Stevenson",
        )
        .await;
    assert_eq!(
        books,
        [
            BookEntry {
                name: "Treasure Island".to_string()
            },
            BookEntry {
                name: "Strange Case of Dr Jekyll and Mr Hyde".to_string()
            }
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_can_find_entries_with_partial_path() {
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
    conductor_batch.exchange_peer_info().await;

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

    // There should be no books created yet.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "S",
        )
        .await;
    assert!(books.is_empty());

    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "S",
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

    // Bob adds a book entry.
    let () = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "add_book_entry",
            ("Stevenson", "Strange Case of Dr Jekyll and Mr Hyde"),
        )
        .await;

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

    // After consistency, both should see each other's entries.
    let books: Vec<BookEntry> = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "S",
        )
        .await;
    assert_eq!(
        books,
        [
            BookEntry {
                name: "Romeo and Juliet".to_string()
            },
            BookEntry {
                name: "Strange Case of Dr Jekyll and Mr Hyde".to_string()
            }
        ]
    );
    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "S",
        )
        .await;
    assert_eq!(
        books,
        [
            BookEntry {
                name: "Romeo and Juliet".to_string()
            },
            BookEntry {
                name: "Strange Case of Dr Jekyll and Mr Hyde".to_string()
            }
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn paths_are_case_sensitive() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Paths]).await;

    let app = conductor.setup_app("paths_app", [&dna]).await.unwrap();

    let alice_cell = app.cells().first().unwrap();

    // There should be no books created yet.
    let books: Vec<BookEntry> = conductor
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "Shakespeare",
        )
        .await;
    assert!(books.is_empty());

    // Alice adds a book entry.
    let () = conductor
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "add_book_entry",
            ("Shakespeare", "Romeo and Juliet"),
        )
        .await;

    // Alice should find her entry when case is same as when entered.
    let books: Vec<BookEntry> = conductor
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

    // Alice should not find her entry when path is all lowercase.
    let books: Vec<BookEntry> = conductor
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "shakespeare",
        )
        .await;
    assert!(books.is_empty(),);

    // Alice should not find her entry when path is all UPPERCASE.
    let books: Vec<BookEntry> = conductor
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_from_author",
            "SHAKESPEARE",
        )
        .await;
    assert!(books.is_empty(),);
}

#[tokio::test(flavor = "multi_thread")]
async fn paths_can_be_created_fully_or_with_path_sharding() {
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
    conductor_batch.exchange_peer_info().await;

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

    // Alice adds a book entry.
    let () = conductor_batch[0]
        .call(
            &alice_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "add_book_entry",
            ("Shakespeare", "Romeo and Juliet"),
        )
        .await;

    await_consistency([&alice_cell, &bob_cell]).await.unwrap();

    // Can find book using path-sharding.
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

    // Can find book manually at a path.
    let books: Vec<BookEntry> = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Paths.coordinator_zome_name()),
            "find_books_at_path",
            // This is the path created by path-sharding the author's name.
            "S.h.a.k.e.s.p.e.a.r.e.Shakespeare",
        )
        .await;
    assert_eq!(
        books,
        [BookEntry {
            name: "Romeo and Juliet".to_string()
        }]
    );
}
