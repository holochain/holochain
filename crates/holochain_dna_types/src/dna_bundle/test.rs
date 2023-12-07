use super::*;
use matches::assert_matches;

#[test]
fn missing_origin_time_is_an_error() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  zomes:
    - name: zome1
      bundled: zome-1.wasm
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
        "#;

    let manifest = serde_yaml::from_str::<DnaManifest>(&manifest_yaml);
    assert!(manifest.is_err());
}

#[test]
fn duplicate_zome_names_is_an_error() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  origin_time: 2022-02-11T23:05:19.470323Z
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome1
      bundled: nested/zome-2.wasm
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
        "#;

    let manifest = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap();
    assert_matches!(
        ValidatedDnaManifest::try_from(manifest),
        Err(DnaError::DuplicateZomeNames(name)) if name.as_str() == "zome1"
    );
}

#[test]
fn dependency_not_pointing_at_integrity_zome_is_error() {
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  origin_time: 2022-02-11T23:05:19.470323Z
  zomes:
    - name: zome1
      bundled: zome-1.wasm
      dependencies:
        - name: zome20
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

    let manifest = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap();
    assert_matches!(
        ValidatedDnaManifest::try_from(manifest),
        Err(DnaError::DanglingZomeDependency(dep, name)) if dep.as_str() == "zome20" && name.as_str() == "zome1"
    );

    // Fails when depending on a coordinator.
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  origin_time: 2022-02-11T23:05:19.470323Z
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
      dependencies:
        - name: zome3
        - name: zome4
    - name: zome3
      path: ../zome-3.wasm
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
        "#;

    let manifest = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap();
    assert_matches!(
        ValidatedDnaManifest::try_from(manifest),
        Err(DnaError::DanglingZomeDependency(dep, name)) if dep.as_str() == "zome4" && name.as_str() == "zome2"
    );

    // Fails when pointing to self.
    let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
  origin_time: 2022-02-11T23:05:19.470323Z
  zomes:
    - name: zome1
      bundled: zome-1.wasm
    - name: zome2
      bundled: nested/zome-2.wasm
      dependencies:
        - name: zome3
        - name: zome2
    - name: zome3
      path: ../zome-3.wasm
coordinator:
  zomes:
    - name: zome4
      bundled: zome-4.wasm
    - name: zome5
      path: ../zome-5.wasm
        "#;

    let manifest = serde_yaml::from_str::<DnaManifest>(&manifest_yaml).unwrap();
    assert_matches!(
        ValidatedDnaManifest::try_from(manifest),
        Err(DnaError::DanglingZomeDependency(dep, name)) if dep.as_str() == "zome2" && name.as_str() == "zome2"
    );
}
