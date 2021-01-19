//! # Manage persistence of setups
//! This module gives basic helpers to save / load your setups
//! in a `.hc` file.
//! This is very much WIP and subject to change.
use std::path::PathBuf;

use crate::config;
use crate::config::CONDUCTOR_CONFIG;

/// Save all setups to the `.hc` file in the `hc_dir` directory.
pub fn save(mut hc_dir: PathBuf, paths: Vec<PathBuf>) -> anyhow::Result<()> {
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

/// Remove setups by their index in the file.
/// You can get the index by calling [`load`].
/// If no setups are passed in then all are deleted.
/// If all setups are deleted the `.hc` file will be removed.
pub fn clean(mut hc_dir: PathBuf, setups: Vec<usize>) -> anyhow::Result<()> {
    let existing = load(hc_dir.clone())?;
    let setups_len = setups.len();
    let to_remove: Vec<_> = if setups.is_empty() {
        existing.iter().collect()
    } else {
        setups.into_iter().filter_map(|i| existing.get(i)).collect()
    };
    let to_remove_len = to_remove.len();
    for p in to_remove {
        if p.exists() && p.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(p) {
                tracing::error!("Failed to remove {} because {:?}", p.display(), e);
            }
        }
    }
    if setups_len == 0 || setups_len == to_remove_len {
        hc_dir.push(".hc");
        if hc_dir.exists() {
            std::fs::remove_file(hc_dir)?;
        }
    }
    Ok(())
}

/// Load setup paths from the `.hc` file.
pub fn load(mut hc_dir: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    hc_dir.push(".hc");
    if hc_dir.exists() {
        let existing = std::fs::read_to_string(hc_dir)?;
        for setup in existing.lines() {
            let path = PathBuf::from(setup);
            let mut config_path = path.clone();
            config_path.push(CONDUCTOR_CONFIG);
            if config_path.exists() {
                paths.push(path);
            } else {
                tracing::error!("Failed to load path {} from existing .hc", path.display());
            }
        }
    }
    Ok(paths)
}

/// Print out the setups contained in the `.hc` file.
pub fn list(hc_dir: PathBuf, verbose: usize) -> anyhow::Result<()> {
    let out = load(hc_dir)?.into_iter().enumerate().try_fold(
        "\nSetups contained in `.hc`\n".to_string(),
        |out, (i, path)| {
            let r = match verbose {
                0 => format!("{}{}: {}\n", out, i, path.display()),
                _ => {
                    let config = config::read_config(path.clone())?;
                    format!(
                        "{}{}: {}\nConductor Config:\n{:?}\n",
                        out,
                        i,
                        path.display(),
                        config
                    )
                }
            };
            anyhow::Result::<_, anyhow::Error>::Ok(r)
        },
    )?;
    msg!("{}", out);
    Ok(())
}
