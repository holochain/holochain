use std::sync::Arc;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_hotswap() {
    let dna_wasms = vec![
        DnaWasm {
            code: Arc::new(Box::new([0])),
        },
        DnaWasm {
            code: Arc::new(Box::new([1])),
        },
        DnaWasm {
            code: Arc::new(Box::new([2])),
        },
        DnaWasm {
            code: Arc::new(Box::new([3])),
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
    let mut dna_def = DnaDefBuilder::default();
    dna_def
        .integrity_zomes(init_integrity.clone())
        .coordinator_zomes(init_coordinators.clone())
        .network_seed("00000000-0000-0000-0000-000000000000".into());
    let dna_def = dna_def.build().unwrap();
    let mut dna = DnaFile::new(dna_def.clone(), dna_wasms.clone())
        .await
        .unwrap();

    let original_dna = dna.clone();

    // Replace coordinator "c".
    let new_dna_wasms = vec![DnaWasm {
        code: Arc::new(Box::new([4])),
    }];
    let new_coordinators = vec![(
        "c".into(),
        CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
            wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
            dependencies: vec!["b".into()],
        })),
    )];
    let old_wasm = dna
        .hot_swap_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
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
    let expect = DnaFile::new(expect_def.clone(), expect_wasms.clone())
        .await
        .unwrap();

    assert_eq!(expect, dna);

    // Add new coordinator "e".
    let new_dna_wasms = vec![DnaWasm {
        code: Arc::new(Box::new([6])),
    }];
    let new_coordinators = vec![(
        "e".into(),
        CoordinatorZomeDef::from(ZomeDef::Wasm(WasmZome {
            wasm_hash: WasmHash::with_data(&new_dna_wasms[0]).await,
            dependencies: vec!["a".into()],
        })),
    )];
    let old_wasm = dna
        .hot_swap_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
        .await
        .unwrap();

    assert_eq!(old_wasm.len(), 0);
    assert_eq!(dna.dna_hash(), original_dna.dna_hash());

    expect_def
        .coordinator_zomes
        .push(new_coordinators[0].clone());
    expect_wasms.push(new_dna_wasms[0].clone());
    let expect = DnaFile::new(expect_def.clone(), expect_wasms.clone())
        .await
        .unwrap();

    assert_eq!(expect, dna);

    // Replace all and add a new coordinator "f".
    let new_dna_wasms = vec![
        DnaWasm {
            code: Arc::new(Box::new([6])),
        },
        DnaWasm {
            code: Arc::new(Box::new([7])),
        },
        DnaWasm {
            code: Arc::new(Box::new([8])),
        },
        DnaWasm {
            code: Arc::new(Box::new([9])),
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
        .hot_swap_coordinators(new_coordinators.clone(), new_dna_wasms.clone())
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

    expect_def.coordinator_zomes = new_coordinators.clone();
    expect_wasms[2] = new_dna_wasms[0].clone();
    expect_wasms[3] = new_dna_wasms[1].clone();
    expect_wasms[4] = new_dna_wasms[2].clone();
    expect_wasms.push(new_dna_wasms[3].clone());
    let expect = DnaFile::new(expect_def, expect_wasms).await.unwrap();

    assert_eq!(expect, dna);
}
