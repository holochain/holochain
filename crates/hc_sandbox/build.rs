fn main() {
    let out_dir: std::path::PathBuf = std::env::var_os("OUT_DIR").unwrap().into();

    let mut target_dir = out_dir.clone();
    target_dir.pop();
    target_dir.pop();
    target_dir.pop();

    let content = format!(
        "const TARGET: &[u8] = &{:?};",
        target_dir.into_os_string().into_encoded_bytes(),
    );

    let mut target_file = out_dir.clone();
    target_file.push("target.rs");

    std::fs::write(target_file, content).unwrap();
}
