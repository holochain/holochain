use assert_cli::Assert;
use tempdir::TempDir;

#[test]
#[ignore] // this test takes too long
fn first_experience_with_holochain_is_a_friendly_one() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("missing-config.toml");
    Assert::main_binary()
        .with_args(&["-c", &path.display().to_string()])
        .fails_with(42)
        .and()
        .stdout()
        .satisfies(
            |o| o.to_string().to_lowercase().contains("please"),
            "startup error is not friendly enough: missing the word \"please\"\n",
        )
        .unwrap();
}

#[test]
#[ignore] // this test takes too long
fn malformed_toml_error_is_friendly() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("malformed-config.toml");
    std::fs::write(&path, "{{ totally [ not ( valid toml").unwrap();
    Assert::main_binary()
        .with_args(&["-c", &path.display().to_string()])
        .fails_with(42)
        .and()
        .stdout()
        .satisfies(
            |o| o.to_string().to_lowercase().contains("please"),
            "startup error is not friendly enough: missing the word \"please\"\n",
        )
        .and()
        .stdout()
        .contains("expected a table key")
        .unwrap();
}

#[test]
#[ignore] // this test takes too long
fn invalid_config_error_is_friendly() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("malformed-config.toml");
    std::fs::write(&path, "[valid]\nbut=\"wrong\"").unwrap();
    Assert::main_binary()
        .with_args(&["-c", &path.display().to_string()])
        .fails_with(42)
        .and()
        .stdout()
        .satisfies(
            |o| o.to_string().to_lowercase().contains("please"),
            "startup error is not friendly enough: missing the word \"please\"\n",
        )
        .and()
        .stdout()
        .contains("missing field")
        .unwrap();
}
