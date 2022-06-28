use mockall::predicate::eq;

use crate::hash_path::path::root_hash;
use crate::prelude::*;

const LINK_TYPE: ScopedLinkType = ScopedLinkType {
    zome_id: ZomeId(0),
    zome_type: LinkType(0),
};

#[test]
/// Test that a root path always doesn't exist until it is ensured.
fn root_ensures() {
    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    set_hdk(mock);

    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    mock.expect_get_links()
        .times(2)
        .returning(|_| Ok(vec![vec![]]));
    mock.expect_create_link()
        .once()
        .with(eq(CreateLinkInput {
            base_address: root_hash().unwrap(),
            target_address: Path::from("foo").path_entry_hash().unwrap().into(),
            zome_id: ZomeId(0),
            link_type: LinkType(0),
            tag: Path::from("foo").make_tag().unwrap(),
            chain_top_ordering: Default::default(),
        }))
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0; 36])));
    set_hdk(mock);

    assert!(!Path::from("foo")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
    Path::from("foo")
        .typed(LINK_TYPE)
        .unwrap()
        .ensure()
        .unwrap();

    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    mock.expect_get_links()
        .once()
        .with(eq(vec![GetLinksInput {
            base_address: root_hash().unwrap(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("foo").make_tag().unwrap()),
        }]))
        .returning(|_| {
            Ok(vec![vec![Link {
                target: Path::from("foo").path_entry_hash().unwrap().into(),
                timestamp: Timestamp::now(),
                tag: Path::from("foo").make_tag().unwrap(),
                create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
            }]])
        });
    set_hdk(mock);
    assert!(Path::from("foo")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
}

#[test]
/// Check the the parent of a path is linked by ensure.
fn parent_path_committed() {
    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    set_hdk(mock);

    let mut mock = MockHdkT::new();
    mock.expect_hash().times(8).returning(hash_entry_mock);
    mock.expect_get_links()
        .times(2)
        .returning(|_| Ok(vec![vec![]]));
    mock.expect_create_link()
        .once()
        .with(eq(CreateLinkInput {
            base_address: Path::from("foo").path_entry_hash().unwrap().into(),
            target_address: Path::from("foo.bar").path_entry_hash().unwrap().into(),
            zome_id: ZomeId(0),
            link_type: LinkType(0),
            tag: Path::from("bar").make_tag().unwrap(),
            chain_top_ordering: Default::default(),
        }))
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0; 36])));
    mock.expect_create_link()
        .once()
        .with(eq(CreateLinkInput {
            base_address: root_hash().unwrap(),
            target_address: Path::from("foo").path_entry_hash().unwrap().into(),
            zome_id: ZomeId(0),
            link_type: LinkType(0),
            tag: Path::from("foo").make_tag().unwrap(),
            chain_top_ordering: Default::default(),
        }))
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0; 36])));
    set_hdk(mock);

    Path::from("foo.bar")
        .typed(LINK_TYPE)
        .unwrap()
        .ensure()
        .unwrap();

    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    set_hdk(mock);

    let mut mock = MockHdkT::new();
    mock.expect_hash().times(12).returning(hash_entry_mock);
    mock.expect_get_links()
        .times(3)
        .returning(|_| Ok(vec![vec![]]));
    mock.expect_create_link()
        .once()
        .with(eq(CreateLinkInput {
            base_address: Path::from("foo.bar").path_entry_hash().unwrap().into(),
            target_address: Path::from("foo.bar.baz").path_entry_hash().unwrap().into(),
            zome_id: ZomeId(0),
            link_type: LinkType(0),
            tag: Path::from("baz").make_tag().unwrap(),
            chain_top_ordering: Default::default(),
        }))
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0; 36])));
    mock.expect_create_link()
        .once()
        .with(eq(CreateLinkInput {
            base_address: Path::from("foo").path_entry_hash().unwrap().into(),
            target_address: Path::from("foo.bar").path_entry_hash().unwrap().into(),
            zome_id: ZomeId(0),
            link_type: LinkType(0),
            tag: Path::from("bar").make_tag().unwrap(),
            chain_top_ordering: Default::default(),
        }))
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0; 36])));
    mock.expect_create_link()
        .once()
        .with(eq(CreateLinkInput {
            base_address: root_hash().unwrap(),
            target_address: Path::from("foo").path_entry_hash().unwrap().into(),
            zome_id: ZomeId(0),
            link_type: LinkType(0),
            tag: Path::from("foo").make_tag().unwrap(),
            chain_top_ordering: Default::default(),
        }))
        .returning(|_| Ok(ActionHash::from_raw_36(vec![0; 36])));
    set_hdk(mock);

    Path::from("foo.bar.baz")
        .typed(LINK_TYPE)
        .unwrap()
        .ensure()
        .unwrap();
}

#[test]
/// Check path exists behavior is correct.
fn paths_exists() {
    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);

    // Return no links.
    mock.expect_get_links().returning(|_| Ok(vec![vec![]]));
    set_hdk(mock);

    // Paths do not exist.
    assert!(!Path::from("foo")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
    assert!(!Path::from("bar")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
    assert!(!Path::from("baz")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
    assert!(!Path::from("foo.bar")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
    assert!(!Path::from("foo.bar.baz")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());

    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);

    // Return links that match the input.
    mock.expect_get_links()
        .once()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("bar").make_tag().unwrap()),
        }]))
        .returning(|_| {
            Ok(vec![vec![Link {
                target: Path::from("foo.bar").path_entry_hash().unwrap().into(),
                timestamp: Timestamp::now(),
                tag: Path::from("bar").make_tag().unwrap(),
                create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
            }]])
        });
    mock.expect_get_links()
        .once()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("baz").make_tag().unwrap()),
        }]))
        .returning(|_| {
            Ok(vec![vec![Link {
                target: Path::from("foo.bar.baz").path_entry_hash().unwrap().into(),
                timestamp: Timestamp::now(),
                tag: Path::from("baz").make_tag().unwrap(),
                create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
            }]])
        });
    set_hdk(mock);

    // Both non-root paths exist now.
    assert!(Path::from("foo.bar")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
    assert!(Path::from("foo.bar.baz")
        .typed(LINK_TYPE)
        .unwrap()
        .exists()
        .unwrap());
}

#[test]
// Check path children behavior is correct.
fn children() {
    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    set_hdk(mock);

    // Create some links to return.
    let foo = Link {
        target: Path::from("foo").path_entry_hash().unwrap().into(),
        timestamp: Timestamp::now(),
        tag: Path::from("foo").make_tag().unwrap(),
        create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
    };
    let foo_bar = Link {
        target: Path::from("foo.bar").path_entry_hash().unwrap().into(),
        timestamp: Timestamp::now(),
        tag: Path::from("bar").make_tag().unwrap(),
        create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
    };
    let foo_bar2 = Link {
        target: Path::from("foo.bar2").path_entry_hash().unwrap().into(),
        timestamp: Timestamp::now(),
        tag: Path::from("bar2").make_tag().unwrap(),
        create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
    };
    let foo_bar_baz = Link {
        target: Path::from("foo.bar.baz").path_entry_hash().unwrap().into(),
        timestamp: Timestamp::now(),
        tag: Path::from("baz").make_tag().unwrap(),
        create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
    };
    let foo_bar2_baz2 = Link {
        target: Path::from("foo.bar2.baz2")
            .path_entry_hash()
            .unwrap()
            .into(),
        timestamp: Timestamp::now(),
        tag: Path::from("baz2").make_tag().unwrap(),
        create_link_hash: ActionHash::from_raw_36(vec![0; 36]),
    };

    // Return links that match the input.
    // ${base} -[${tag}]-> ${target}
    let mut mock = MockHdkT::new();
    mock.expect_hash().returning(hash_entry_mock);
    // ROOT -[foo]-> foo
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: root_hash().unwrap(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("foo").make_tag().unwrap()),
        }]))
        .returning({
            let foo = foo.clone();
            move |_| Ok(vec![vec![foo.clone()]])
        });
    // foo -[bar]-> foo.bar
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("bar").make_tag().unwrap()),
        }]))
        .returning({
            let foo_bar = foo_bar.clone();
            move |_| Ok(vec![vec![foo_bar.clone()]])
        });
    // foo -[bar2]-> foo.bar2
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("bar2").make_tag().unwrap()),
        }]))
        .returning({
            let foo_bar2 = foo_bar2.clone();
            move |_| Ok(vec![vec![foo_bar2.clone()]])
        });
    // foo.bar -[baz]-> foo.bar.baz
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("baz").make_tag().unwrap()),
        }]))
        .returning({
            let foo_bar_baz = foo_bar_baz.clone();
            move |_| Ok(vec![vec![foo_bar_baz.clone()]])
        });
    // foo.bar2 -[baz2]-> foo.bar2.baz2
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar2").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: Some(Path::from("baz2").make_tag().unwrap()),
        }]))
        .returning({
            let foo_bar2_baz2 = foo_bar2_baz2.clone();
            move |_| Ok(vec![vec![foo_bar2_baz2.clone()]])
        });
    // foo -[]-> (foo.bar, foo.bar2)
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: None,
        }]))
        .returning(move |_| Ok(vec![vec![foo_bar.clone(), foo_bar2.clone()]]));
    // foo.bar -[]-> foo.bar.baz
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: None,
        }]))
        .returning(move |_| Ok(vec![vec![foo_bar_baz.clone()]]));
    // foo.bar2 -[]-> foo.bar2.baz2
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar2").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: None,
        }]))
        .returning(move |_| Ok(vec![vec![foo_bar2_baz2.clone()]]));
    // foo.bar.baz -[]-> ()
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar.baz").path_entry_hash().unwrap().into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: None,
        }]))
        .returning(|_| Ok(vec![vec![]]));
    // foo.bar2.baz2 -[]-> ()
    mock.expect_get_links()
        .with(eq(vec![GetLinksInput {
            base_address: Path::from("foo.bar2.baz2")
                .path_entry_hash()
                .unwrap()
                .into(),
            link_type: LinkTypeFilter::single_type(0.into(), 0.into()),
            tag_prefix: None,
        }]))
        .returning(|_| Ok(vec![vec![]]));
    set_hdk(mock);

    // These have no children.
    assert_eq!(
        Path::from("foo.bar.baz")
            .typed(LINK_TYPE)
            .unwrap()
            .children_paths()
            .unwrap(),
        vec![]
    );

    assert_eq!(
        Path::from("foo.bar2.baz2")
            .typed(LINK_TYPE)
            .unwrap()
            .children_paths()
            .unwrap(),
        vec![]
    );

    // These have on child.
    assert_eq!(
        Path::from("foo.bar")
            .typed(LINK_TYPE)
            .unwrap()
            .children_paths()
            .unwrap(),
        vec![Path::from("foo.bar.baz").typed(LINK_TYPE).unwrap()]
    );

    assert_eq!(
        Path::from("foo.bar2")
            .typed(LINK_TYPE)
            .unwrap()
            .children_paths()
            .unwrap(),
        vec![Path::from("foo.bar2.baz2").typed(LINK_TYPE).unwrap()]
    );

    // This has two children.
    assert_eq!(
        Path::from("foo")
            .typed(LINK_TYPE)
            .unwrap()
            .children_paths()
            .unwrap(),
        vec![
            Path::from("foo.bar").typed(LINK_TYPE).unwrap(),
            Path::from("foo.bar2").typed(LINK_TYPE).unwrap(),
        ]
    );
}

// Utility to create correct hashing for mocks.
fn hash_entry_mock(input: HashInput) -> ExternResult<HashOutput> {
    match input {
        HashInput::Entry(e) => Ok(HashOutput::Entry(EntryHash::with_data_sync(&e))),
        _ => todo!(),
    }
}
