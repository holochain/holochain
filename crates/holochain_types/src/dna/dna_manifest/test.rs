use super::*;

#[test]
fn can_deserialize_dna_manifest_integrity_zomes() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
    - name: zome3
      path: ../zome-3.wasm
      "#;

    let _manifest: DnaManifest = serde_yaml::from_str(&manifest_yaml).unwrap();
}

#[test]
fn can_deserialize_dna_manifest_all_zomes() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
    - name: zome3
      path: ../zome-3.wasm
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
        "#;

    let _manifest: DnaManifest = serde_yaml::from_str(&manifest_yaml).unwrap();
}

#[test]
fn deserialize_dna_manifest_coordinator_only() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity: 
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  zomes: ~
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
        "#;

    serde_yaml::from_str::<DnaManifest>(&manifest_yaml)
        .expect_err("This should fail because integrity zomes are required");
}

#[test]
fn rejects_manifest_with_unknown_fields() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
    - name: zome3
      path: ../zome-3.wasm
not_a_real_field: ~"#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `not_a_real_field`"),
        "Should have rejected unknown field but actually got: {}",
        err.to_string(),
    );
}

#[test]
fn rejects_manifest_with_coordinators_defined_under_integrity() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
    - name: zome3
      path: ../zome-3.wasm
  # Should be indented left once, this is actually nested under `integrity`
  coordinator:
    zomes:
      - name: zome4
        bundled: zome-4.wasm
      - name: zome5
        path: ../zome-5.wasm"#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `coordinator`"),
        "Should have rejected coordinator zomes nested under integrity but actually got: {}",
        err.to_string(),
    );
}

#[test]
fn rejects_manifest_with_integrity_defined_under_coordinators() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
  # Should be indented left once, this is actually nested under `coordinator`
  integrity:
    network_seed: blablabla
    origin_time: 2022-02-11T23:29:00.789576Z
    properties: ~
    zomes:
      - name: zome1
        bundled: zome-1.wasm
      - name: zome2
        bundled: nested/zome-2.wasm
      - name: zome3
        path: ../zome-3.wasm"#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `integrity`"),
        "Should have rejected integrity zomes nested under coordinators but actually got: {}",
        err.to_string(),
    );
}

#[test]
fn rejects_manifest_with_unknown_integrity_fields() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  not_a_real_field: ~
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
    - name: zome3
      path: ../zome-3.wasm
"#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `not_a_real_field`"),
        "Should have rejected unknown field but actually got: {}",
        err.to_string(),
    );
}

#[test]
fn rejects_manifest_with_unknown_coordinator_fields() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
coordinator:
  not_a_real_field: ~
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
        "#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `not_a_real_field`"),
        "Should have rejected unknown field but actually got: {}",
        err.to_string(),
    );
}

#[test]
fn rejects_manifest_with_unknown_zome_fields_in_integrity() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  network_seed: blablabla
  origin_time: 2022-02-11T23:29:00.789576Z
  properties: ~
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
      not_a_real_field: ~
    - name: zome3
      path: ../zome-3.wasm
"#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `not_a_real_field`"),
        "Should have rejected unknown field but actually got: {}",
        err.to_string(),
    );
}

#[test]
fn rejects_manifest_with_unknown_zome_field_in_coordinator() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
      not_a_real_field: ~
        "#;

    let err = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `not_a_real_field`"),
        "Should have rejected unknown field but actually got: {}",
        err.to_string(),
    );
}
