use super::*;

#[test]
fn can_deserialize_dna_manifest_zomes() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
uid: blablabla
origin_time: 2022-02-11T23:29:00.789576Z
properties:
  some: 42
  props: yay
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
fn can_deserialize_dna_manifest_integrity_zomes() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
uid: blablabla
origin_time: 2022-02-11T23:29:00.789576Z
properties: ~
zomes:
  integrity:
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
uid: blablabla
origin_time: 2022-02-11T23:29:00.789576Z
properties: ~
zomes:
  integrity:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
    - name: zome3
      path: ../zome-3.wasm
  coordinator:
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
uid: blablabla
origin_time: 2022-02-11T23:29:00.789576Z
properties: ~
zomes:
  integrity: ~
  coordinator:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
        "#;

    serde_yaml::from_str::<DnaManifest>(&manifest_yaml)
        .expect_err("This should fail because integrity zomes are required");
}
