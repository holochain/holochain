use assert_cli::Assert;
use tempdir::TempDir;

#[test]
fn first_experience_with_holochain_is_a_friendly_one() {
    let tmp = TempDir::new("").unwrap();
    let path = tmp.path().join("config.toml").display().to_string();
    Assert::main_binary()
        .with_args(&["-c", &path])
        .fails_with(42)
        .and()
        .stdout()
        .contains("Please")
        .unwrap();
}
