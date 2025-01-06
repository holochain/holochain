use std::io::{BufRead, BufReader, Error as IoError, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use tempfile::TempDir;

const BINARY_NAME: &str = "hc-conductor-config";

/// Test context provides a controlled environment for testing the cli
/// and provide utilities for running commands and reading configuration files
#[derive(Debug)]
struct TestContext {
    temp_dir: TempDir,
    binary_path: PathBuf,
}

impl TestContext {
    fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");

        let binary_path = get_binary_path();

        Self {
            temp_dir,
            binary_path,
        }
    }

    fn run_create_config_command(&self, password: &str, args: &[&str]) -> std::io::Result<Child> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.current_dir(&self.temp_dir)
            .env("RUST_BACKTRACE", "1")
            .arg("--piped")
            .arg("create")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        pipe_password_to_process(&mut cmd, password)
    }

    fn get_config_root_path(&self, process: &mut Child) -> PathBuf {
        let stdout = process.stdout.take().expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            let line = line.expect("Failed to read line");
            println!("@@@-{line}-@@@");

            if let Some(index) = line.find("ConfigRootPath") {
                let config_root_path = line[index..]
                    .trim()
                    .strip_prefix("ConfigRootPath(\"")
                    .expect("Invalid config path format")
                    .strip_suffix("\")")
                    .expect("Invalid config path format");

                return PathBuf::from(config_root_path);
            }
        }

        panic!("Config root path not found in process output");
    }
}

fn get_binary_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("Failed to get parent directory")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf();

    println!("Current directory: {:?}", path);

    path.push(if cfg!(windows) {
        format!("{}.exe", BINARY_NAME)
    } else {
        BINARY_NAME.to_string()
    });

    println!("Looking for binary at: {:?}", path);

    if !path.exists() {
        panic!(
            "Binary not found at {:?}. Have you run `cargo build`?",
            path
        );
    }
    path
}

fn pipe_password_to_process(cmd: &mut Command, password: &str) -> Result<Child, IoError> {
    let mut child = cmd.stdin(Stdio::piped()).spawn()?;

    // take ownership of stdin and write password
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
    }

    Ok(child)
}

#[test]
fn test_config_files_are_generated() {
    let ctx = TestContext::new();
    let mut cli_process = ctx
        .run_create_config_command("test-phrase", &[])
        .expect("Failed to run create command");

    let config_path = ctx.get_config_root_path(&mut cli_process);
    assert!(
        config_path.exists(),
        "Config root directory does not exist at {:?}",
        config_path
    );

    let conductor_config = config_path.join("conductor-config.yaml");
    let keystore_config = config_path.join("ks").join("lair-keystore-config.yaml");
    assert!(
        conductor_config.exists(),
        "conductor-config.yaml not found at {:?}",
        conductor_config
    );
    assert!(
        keystore_config.exists(),
        "lair-keystore-config.yaml not found at {:?}",
        keystore_config
    );
}
