use std::path::PathBuf;

use holochain_conductor_api::conductor::paths::ConfigRootPath;

/// Save all sandboxes to the `.hc` file in the `hc_dir` directory.
pub fn save(mut hc_dir: PathBuf, paths: Vec<ConfigRootPath>) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(&hc_dir)?;
    hc_dir.push(".hc");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(hc_dir)?;

    for path in paths {
        writeln!(file, "{}", path.display())?;
    }
    Ok(())
}
