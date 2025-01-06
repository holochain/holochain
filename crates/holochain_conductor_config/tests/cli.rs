use std::path::PathBuf;

const BINARY_NAME: &str = "holochain_conductor_config";

fn get_binary_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf();

    path.push(if cfg!(windows) {
        format!("{}.exe", BINARY_NAME)
    } else {
        BINARY_NAME.to_string()
    });

    if !path.exists() {
        panic!(
            "Binary not found at {:?}. Have you run `cargo build`?",
            path
        );
    }

    path
}
