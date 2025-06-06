use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_update_coordinators() {
    let dna_wasms = vec![
        DnaWasm {
            code: vec![0].into(),
        },
        DnaWasm {
            code: vec![1].into(),
        },
        DnaWasm {
            code: vec![2].into(),
        },
        DnaWasm {
            code: vec![3].into(),
        },
    ];
    let init_integrity = vec![
        (
            "a".into(),
            IntegrityZomeDef::from_hash(WasmHash::with_data(&dna_wasms[0]).await),
        ),
        (
            "b".into(),
            IntegrityZomeDef::from_hash(WasmHash::with_data(&dna_wasms[1]).await),
        ),
    ];
    let init_coordinators = vec![
        (
            "c".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&dna_wasms[2]).await,
                dependencies: vec!["b".into()],
            })),
        ),
        (
            "d".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&dna_wasms[3]).await,
                dependencies: vec!["b".into(), "a".into()],
            })),
        ),
    ];
    let mut dna_modifiers = DnaModifiersBuilder::default();
    dna_modifiers.network_seed("00000000-0000-0000-0000-000000000000".into());
    let mut dna_def = DnaDefBuilder::default();
    dna_def
        .integrity_zomes(init_integrity.clone())
        .coordinator_zomes(init_coordinators.clone())
        .modifiers(dna_modifiers.build().unwrap());
    let dna_def = dna_def.build().unwrap();
    let mut dna = DnaFile::new(dna_def.clone(), dna_wasms.clone()).await;

    let original_dna = dna.clone();

    // Replace coordinator "c".
    let new_dna_wasms = vec![DnaWasm {
        code: vec![4].into(),
    }];
    let new_coordinators = vec![(
        "c".into(),
        CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
            wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
            dependencies: vec!["b".into()],
        })),
    )];
    let old_wasm = dna
        .update_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
        .await
        .unwrap();

    assert_eq!(old_wasm.len(), 1);
    assert_eq!(
        old_wasm[0],
        init_coordinators[0].1.wasm_hash(&"c".into()).unwrap()
    );
    assert_eq!(dna.dna_hash(), original_dna.dna_hash());

    let mut expect_def = dna_def.clone();
    expect_def.coordinator_zomes[0] = new_coordinators[0].clone();
    let mut expect_wasms = dna_wasms.clone();
    expect_wasms[2] = new_dna_wasms[0].clone();
    let expect = DnaFile::new(expect_def.clone(), expect_wasms.clone()).await;

    assert_eq!(expect, dna);

    // Add new coordinator "e"
    let new_dna_wasms = vec![DnaWasm {
        code: vec![6].into(),
    }];
    let new_coordinators = vec![(
        "e".into(),
        CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
            wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
            dependencies: vec!["a".into()],
        })),
    )];
    let old_wasm = dna
        .update_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
        .await
        .unwrap();

    assert_eq!(old_wasm.len(), 0);
    assert_eq!(dna.dna_hash(), original_dna.dna_hash());

    expect_def
        .coordinator_zomes
        .push(new_coordinators[0].clone());
    expect_wasms.push(new_dna_wasms[0].clone());
    let expect = DnaFile::new(expect_def.clone(), expect_wasms.clone()).await;

    assert_eq!(expect, dna);

    // Replace all and add a new coordinator "f".
    let new_dna_wasms = vec![
        DnaWasm {
            code: vec![6].into(),
        },
        DnaWasm {
            code: vec![7].into(),
        },
        DnaWasm {
            code: vec![8].into(),
        },
        DnaWasm {
            code: vec![9].into(),
        },
    ];
    let new_coordinators = vec![
        (
            "c".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
                dependencies: vec!["a".into()],
            })),
        ),
        (
            "d".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&new_dna_wasms[1]).await,
                dependencies: vec!["a".into()],
            })),
        ),
        (
            "e".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&new_dna_wasms[2]).await,
                dependencies: vec!["a".into()],
            })),
        ),
        (
            "f".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&new_dna_wasms[3]).await,
                dependencies: vec!["a".into()],
            })),
        ),
    ];
    let old_wasm = dna
        .update_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
        .await
        .unwrap();

    assert_eq!(old_wasm.len(), 3);
    assert_eq!(
        old_wasm[0],
        expect_def.coordinator_zomes[0]
            .1
            .wasm_hash(&"c".into())
            .unwrap()
    );
    assert_eq!(
        old_wasm[1],
        init_coordinators[1].1.wasm_hash(&"d".into()).unwrap()
    );
    assert_eq!(
        old_wasm[2],
        expect_def.coordinator_zomes[2]
            .1
            .wasm_hash(&"e".into())
            .unwrap()
    );
    assert_eq!(dna.dna_hash(), original_dna.dna_hash());

    expect_def.coordinator_zomes.clone_from(&new_coordinators);
    expect_wasms[2] = new_dna_wasms[0].clone();
    expect_wasms[3] = new_dna_wasms[1].clone();
    expect_wasms[4] = new_dna_wasms[2].clone();
    expect_wasms.push(new_dna_wasms[3].clone());
    let expect = DnaFile::new(expect_def, expect_wasms).await;

    assert_eq!(expect, dna);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_coordinators_checks_deps() {
    let dna_wasms = vec![
        DnaWasm {
            code: vec![0].into(),
        },
        DnaWasm {
            code: vec![1].into(),
        },
        DnaWasm {
            code: vec![2].into(),
        },
        DnaWasm {
            code: vec![3].into(),
        },
    ];
    let init_integrity = vec![
        (
            "a".into(),
            IntegrityZomeDef::from_hash(WasmHash::with_data(&dna_wasms[0]).await),
        ),
        (
            "b".into(),
            IntegrityZomeDef::from_hash(WasmHash::with_data(&dna_wasms[1]).await),
        ),
    ];
    let init_coordinators = vec![
        (
            "c".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&dna_wasms[2]).await,
                dependencies: vec!["b".into()],
            })),
        ),
        (
            "d".into(),
            CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
                wasm_hash: WasmHash::with_data(&dna_wasms[3]).await,
                dependencies: vec!["b".into(), "a".into()],
            })),
        ),
    ];
    let mut dna_modifiers = DnaModifiersBuilder::default();
    dna_modifiers.network_seed("00000000-0000-0000-0000-000000000000".into());
    let mut dna_def = DnaDefBuilder::default();
    dna_def
        .integrity_zomes(init_integrity.clone())
        .coordinator_zomes(init_coordinators.clone())
        .modifiers(dna_modifiers.build().unwrap());
    let dna_def = dna_def.build().unwrap();
    let mut dna = DnaFile::new(dna_def.clone(), dna_wasms.clone()).await;

    // Replace coordinator "c" with coordinator that has a dangling reference.
    let new_dna_wasms = vec![DnaWasm {
        code: vec![4].into(),
    }];
    let new_coordinators = vec![(
        "c".into(),
        CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
            wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
            dependencies: vec!["z".into()],
        })),
    )];
    let err = dna
        .update_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
        .await
        .expect_err("Update didn't catch dangling dependency");

    assert!(matches!(err, DnaError::DanglingZomeDependency(_, _)));

    // Add new coordinator "e" with coordinator that has a dangling reference.
    let new_dna_wasms = vec![DnaWasm {
        code: vec![5].into(),
    }];
    let new_coordinators = vec![(
        "e".into(),
        CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
            wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
            dependencies: vec!["z".into()],
        })),
    )];
    let err = dna
        .update_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
        .await
        .expect_err("Update coordinators didn't catch dangling dependency");

    assert!(matches!(err, DnaError::DanglingZomeDependency(_, _)));
}
