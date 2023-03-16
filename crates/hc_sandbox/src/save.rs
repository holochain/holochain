//! # Manage persistence of sandboxes
//! This module gives basic helpers to save / load your sandboxes
//! to /from a `.hc` file.
//! This is very much WIP and subject to change.
use std::path::Path;
use std::path::PathBuf;

use crate::config;
use crate::config::CONDUCTOR_CONFIG;
use once_cell::sync::Lazy;

/// Save all sandboxes to the `.hc` file in the `hc_dir` directory.
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

/// Remove sandboxes by their index in the file.
/// You can get the index by calling [`load`].
/// If no sandboxes are passed in then all are deleted.
/// If all sandboxes are deleted the `.hc` file will be removed.
pub fn clean(mut hc_dir: PathBuf, sandboxes: Vec<usize>) -> anyhow::Result<()> {
    let existing = load(hc_dir.clone())?;
    let sandboxes_len = sandboxes.len();
    let to_remove: Vec<_> = if sandboxes.is_empty() {
        existing.iter().collect()
    } else {
        sandboxes
            .into_iter()
            .filter_map(|i| existing.get(i))
            .collect()
    };
    let to_remove_len = to_remove.len();
    for p in to_remove {
        if p.exists() && p.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(p) {
                tracing::error!("Failed to remove {} because {:?}", p.display(), e);
            }
        }
    }
    if sandboxes_len == 0 || sandboxes_len == to_remove_len {
        for entry in std::fs::read_dir(&hc_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(s) = entry.file_name().to_str() {
                    if s.starts_with(".hc_live_") {
                        std::fs::remove_file(entry.path())?;
                    }
                }
            }
        }
        hc_dir.push(".hc");
        if hc_dir.exists() {
            std::fs::remove_file(hc_dir)?;
        }
    }
    Ok(())
}

/// Load sandbox paths from the `.hc` file.
pub fn load(mut hc_dir: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    hc_dir.push(".hc");
    if hc_dir.exists() {
        let existing = std::fs::read_to_string(hc_dir)?;
        for sandbox in existing.lines() {
            let path = PathBuf::from(sandbox);
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

/// Print out the sandboxes contained in the `.hc` file.
pub fn list(hc_dir: PathBuf, verbose: usize) -> anyhow::Result<()> {
    let out = load(hc_dir)?.into_iter().enumerate().try_fold(
        "\nSandboxes contained in `.hc`\n".to_string(),
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

static FILE_LOCKS: Lazy<tokio::sync::Mutex<Vec<usize>>> =
    Lazy::new(|| tokio::sync::Mutex::new(Vec::new()));

/// Lock this setup as running live and advertise the port
pub async fn lock_live(mut hc_dir: PathBuf, path: &Path, port: u16) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(&hc_dir)?;
    let paths = load(hc_dir.clone())?;
    let index = match paths.into_iter().enumerate().find(|p| p.1 == path) {
        Some((i, _)) => i,
        None => return Ok(()),
    };
    hc_dir.push(format!(".hc_live_{}", index));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(hc_dir)
    {
        Ok(mut file) => {
            writeln!(file, "{}", port)?;
            let mut lock = FILE_LOCKS.lock().await;
            lock.push(index);
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::AlreadyExists => {}
            _ => return Err(e.into()),
        },
    }

    Ok(())
}

/// For each registered setup, if it has a lockfile, return the port of the running conductor,
/// otherwise return None.
/// The resulting Vec has the same number of elements as lines in the `.hc` file
pub fn load_ports(hc_dir: PathBuf) -> anyhow::Result<Vec<Option<u16>>> {
    let mut ports = Vec::new();
    let paths = load(hc_dir.clone())?;
    for (i, _) in paths.into_iter().enumerate() {
        let mut hc = hc_dir.clone();
        hc.push(format!(".hc_live_{}", i));
        if hc.exists() {
            let live = std::fs::read_to_string(hc)?;
            let p = live.lines().next().and_then(|l| l.parse::<u16>().ok());
            ports.push(p)
        } else {
            ports.push(None);
        }
    }
    Ok(ports)
}

/// Same as load_ports but only returns ports for paths passed in.
pub fn find_ports(hc_dir: PathBuf, paths: &[PathBuf]) -> anyhow::Result<Vec<Option<u16>>> {
    let mut ports = Vec::new();
    let all_paths = load(hc_dir.clone())?;
    for path in paths {
        let index = all_paths.iter().position(|p| p == path);
        match index {
            Some(i) => {
                let mut hc = hc_dir.clone();
                hc.push(format!(".hc_live_{}", i));
                if hc.exists() {
                    let live = std::fs::read_to_string(hc)?;
                    let p = live.lines().next().and_then(|l| l.parse::<u16>().ok());
                    ports.push(p)
                } else {
                    ports.push(None);
                }
            }
            None => ports.push(None),
        }
    }
    Ok(ports)
}

/// Remove all lockfiles, releasing all locked ports.
pub async fn release_ports(hc_dir: PathBuf) -> anyhow::Result<()> {
    let files = FILE_LOCKS.lock().await;
    for file in files.iter() {
        let mut hc = hc_dir.clone();
        hc.push(format!(".hc_live_{}", file));
        if hc.exists() {
            std::fs::remove_file(hc)?;
        }
    }
    Ok(())
}
