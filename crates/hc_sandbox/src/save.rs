//! # Manage persistence of sandboxes
//!
//! This module gives basic helpers to save / load your sandboxes
//! to / from a `.hc` file.
//! This is very much WIP and subject to change.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::cmds::Existing;
use holochain_conductor_api::conductor::paths::{ConfigFilePath, ConfigRootPath};

/// Save all sandbox paths to the `.hc` file in the `hc_dir` directory.
pub fn save(hc_dir: PathBuf, paths: Vec<ConfigRootPath>) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(&hc_dir).map_err(|err| {
        std::io::Error::new(
            err.kind(),
            format!("Failed to create directory '{}': {}", hc_dir.display(), err),
        )
    })?;
    let hc_file = hc_dir.join(".hc");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(hc_file)
        .map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "Failed to create `.hc` file at '{}': {}",
                    hc_dir.display(),
                    err
                ),
            )
        })?;
    for path in paths {
        writeln!(file, "{}", path.display())?;
    }
    Ok(())
}

/// Remove sandbox paths from the `.hc` file and attempt to delete the sandbox folders.
///
/// If no sandbox paths remain in the `.hc` file, then the `.hc`, `.hc_auth`, and all `.hc_live*`
/// files will be removed from `hc_dir`.
///
/// Returns the number of removed paths from `.hc`.
pub fn remove(hc_dir: PathBuf, existing: Existing) -> std::io::Result<usize> {
    let sandboxes = load(hc_dir.clone())?;
    // Determine sandbox paths to delete
    let mut to_remove_indices: Vec<usize> = Vec::new();
    if existing.all {
        to_remove_indices = (0..sandboxes.len()).collect();
    } else {
        existing
            .indices
            .into_iter()
            .for_each(|i| match sandboxes.get(i) {
                None => msg!("Warning: Provided index is out of range: {}", i),
                Some(Err(path)) => msg!("Warning: Missing sandbox {}:{}", i, path.display()),
                Some(Ok(_)) => to_remove_indices.push(i),
            });
    }
    // Determine remaining paths
    let indices_to_remove: HashSet<_> = to_remove_indices.iter().collect();
    let remaining: Vec<ConfigRootPath> = sandboxes
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if indices_to_remove.contains(&i) {
                return None;
            }
            let Ok(item) = item else {
                return None;
            };
            Some(ConfigRootPath::from(item.clone()))
        })
        .collect();
    // Attempt to delete each sandbox
    for i in to_remove_indices.iter() {
        let Some(Ok(p)) = sandboxes.get(*i) else {
            continue;
        };
        if p.exists() && p.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(p) {
                tracing::error!("Failed to remove {} because {:?}", p.display(), e);
                msg!(
                    "Failed to remove sandbox {}:{}\nReason: {}",
                    i,
                    p.display(),
                    e
                );
            }
        }
    }
    // Erase .hc file
    let hc_file = hc_dir.join(".hc");
    if hc_file.exists() {
        std::fs::remove_file(&hc_file).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "Failed to remove '.hc' at {}\nReason: {}",
                    hc_dir.display(),
                    err
                ),
            )
        })?;
    }
    // If some valid paths remain, write a new .hc file.
    // Otherwise, delete all files created by hc-sandbox in `hc_dir`
    if !remaining.is_empty() {
        save(hc_dir, remaining.clone())?;
    } else {
        // Erase all .hc_live* files
        for entry in std::fs::read_dir(&hc_dir)? {
            let Ok(entry) = entry else { continue };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_file() {
                if let Some(s) = entry.file_name().to_str() {
                    if s.starts_with(".hc_live_") {
                        std::fs::remove_file(entry.path()).map_err(|err| {
                            std::io::Error::new(
                                err.kind(),
                                format!(
                                    "Failed to remove '{}' at {}\nReason: {}",
                                    s,
                                    hc_dir.display(),
                                    err
                                ),
                            )
                        })?
                    }
                }
            }
        }
        // Erase .hc_auth file
        let hc_auth = hc_dir.join(".hc_auth");
        if hc_auth.exists() {
            std::fs::remove_file(&hc_auth).map_err(|err| {
                std::io::Error::new(
                    err.kind(),
                    format!(
                        "Failed to remove '.hc_auth' at {}\nReason: {}",
                        hc_dir.display(),
                        err
                    ),
                )
            })?;
        }
    }

    Ok(sandboxes.len() - remaining.len())
}

/// Load sandbox paths from the `.hc` file.
pub fn load(hc_dir: PathBuf) -> std::io::Result<Vec<Result<PathBuf, PathBuf>>> {
    let mut paths = Vec::new();
    let hc_file = hc_dir.join(".hc");
    if hc_file.exists() {
        let existing = std::fs::read_to_string(hc_file).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!("Failed to read file at '{}': {}", hc_dir.display(), err),
            )
        })?;
        for sandbox in existing.lines() {
            let path = PathBuf::from(sandbox);
            let config_file_path = ConfigFilePath::from(ConfigRootPath::from(path.clone()));
            if config_file_path.as_ref().exists() {
                paths.push(Ok(path));
            } else {
                tracing::error!("Failed to load path {} from existing .hc", path.display());
                paths.push(Err(path));
            }
        }
    }
    Ok(paths)
}

/// Print out the sandboxes contained in the `.hc` file.
pub fn list(hc_dir: PathBuf, verbose: bool) -> std::io::Result<()> {
    let sandboxes = load(hc_dir.clone())?;
    if sandboxes.is_empty() {
        msg!("No sandboxes contained in {}", hc_dir.join(".hc").display());
        return Ok(());
    }
    let out = sandboxes.into_iter().enumerate().try_fold(
        "\nSandboxes contained in `.hc`\n".to_string(),
        |out, (i, result)| {
            let r = match (result, verbose) {
                (Err(path), _) => format!("{}{}: Missing ({})\n", out, i, path.display()),
                (Ok(path), false) => format!("{}{}: {}\n", out, i, path.display()),
                (Ok(path), true) => {
                    let config = holochain_conductor_config::config::read_config(
                        ConfigRootPath::from(path.clone()),
                    )
                    .map_err(std::io::Error::other)?;
                    format!(
                        "{}{}: {}\nConductor Config:\n{:?}\n",
                        out,
                        i,
                        path.display(),
                        config
                    )
                }
            };
            std::io::Result::Ok(r)
        },
    )?;
    msg!("{}", out);
    Ok(())
}

fn get_file_locks() -> &'static tokio::sync::Mutex<Vec<usize>> {
    static FILE_LOCKS: OnceLock<tokio::sync::Mutex<Vec<usize>>> = OnceLock::new();

    FILE_LOCKS.get_or_init(|| tokio::sync::Mutex::new(Vec::new()))
}

/// Lock this setup as running live and advertise the port.
pub async fn lock_live(mut hc_dir: PathBuf, path: &Path, port: u16) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(&hc_dir)?;
    let paths = load(hc_dir.clone())?;
    let index = match paths
        .into_iter()
        .enumerate()
        .find(|p| p.1 == Ok(path.to_path_buf()))
    {
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
            let mut lock = get_file_locks().lock().await;
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
/// The resulting Vec has the same number of elements as lines in the `.hc` file.
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
        let index = all_paths.iter().position(|p| *p == Ok(path.to_path_buf()));
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
    let files = get_file_locks().lock().await;
    for file in files.iter() {
        let mut hc = hc_dir.clone();
        hc.push(format!(".hc_live_{}", file));
        if hc.exists() {
            std::fs::remove_file(hc)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_conductor_api::conductor::paths::ConfigRootPath;
    use std::fs;

    #[test]
    fn test_save_single_path() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create a test sandbox directory
        let sandbox_dir = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox_dir)?;

        // Save the path
        let paths = vec![ConfigRootPath::from(sandbox_dir.clone())];
        save(hc_dir.clone(), paths)?;

        // Verify the .hc file was created and contains the correct path
        let hc_file = hc_dir.join(".hc");
        assert!(hc_file.exists());
        let content = fs::read_to_string(hc_file)?;
        assert_eq!(content.trim(), sandbox_dir.to_string_lossy());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_save_multiple_paths() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Save the paths
        let paths = vec![
            ConfigRootPath::from(sandbox1.clone()),
            ConfigRootPath::from(sandbox2.clone()),
        ];
        save(hc_dir.clone(), paths)?;

        // Verify the .hc file was created and contains the correct paths
        let hc_file = hc_dir.join(".hc");
        assert!(hc_file.exists());
        let content = fs::read_to_string(hc_file)?;
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], sandbox1.to_string_lossy());
        assert_eq!(lines[1], sandbox2.to_string_lossy());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_save_to_nonexistent_directory() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("nonexistent");

        // Create a test sandbox directory
        let sandbox_dir = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox_dir)?;

        // Save the path to a nonexistent directory (should create it)
        let paths = vec![ConfigRootPath::from(sandbox_dir.clone())];
        save(hc_dir.clone(), paths)?;

        // Verify the directory and .hc file were created
        assert!(hc_dir.exists());
        let hc_file = hc_dir.join(".hc");
        assert!(hc_file.exists());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_save_append() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Save the first path
        let paths1 = vec![ConfigRootPath::from(sandbox1.clone())];
        save(hc_dir.clone(), paths1)?;

        // Save the second path (should append)
        let paths2 = vec![ConfigRootPath::from(sandbox2.clone())];
        save(hc_dir.clone(), paths2)?;

        // Verify the .hc file contains both paths
        let hc_file = hc_dir.join(".hc");
        let content = fs::read_to_string(hc_file)?;
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], sandbox1.to_string_lossy());
        assert_eq!(lines[1], sandbox2.to_string_lossy());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_load_empty_directory() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Load from an empty directory (no .hc file)
        let paths = load(hc_dir.clone())?;

        // Verify the result is an empty vector
        assert!(paths.is_empty());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_load_valid_paths() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Load the paths
        let loaded_paths = load(hc_dir.clone())?;

        // Verify the loaded paths
        assert_eq!(loaded_paths.len(), 2);
        assert_eq!(loaded_paths[0], Ok(sandbox1.clone()));
        assert_eq!(loaded_paths[1], Ok(sandbox2.clone()));

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_load_after_directory_removal() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Remove one of the directories
        fs::remove_dir_all(&sandbox2)?;

        // Load the paths
        let loaded_paths = load(hc_dir.clone())?;

        // Verify the loaded paths
        assert_eq!(loaded_paths.len(), 2);
        assert_eq!(loaded_paths[0], Ok(sandbox1.clone()));
        assert_eq!(loaded_paths[1], Err(sandbox2.clone()));

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_load_after_file_removal() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Remove the config files but keep the directories
        fs::remove_file(config_file_path2.as_ref())?;

        // Load the paths
        let loaded_paths = load(hc_dir.clone())?;

        // Verify the loaded paths
        assert_eq!(loaded_paths.len(), 2);
        assert_eq!(loaded_paths[0], Ok(sandbox1.clone()));
        assert_eq!(loaded_paths[1], Err(sandbox2.clone()));

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_remove_specific_sandboxes() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        let sandbox3 = test_dir.join("sandbox3");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;
        fs::create_dir_all(&sandbox3)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        let config_path3 = ConfigRootPath::from(sandbox3.clone());
        let config_file_path3 = ConfigFilePath::from(config_path3.clone());
        fs::create_dir_all(config_file_path3.as_ref().parent().unwrap())?;
        fs::write(config_file_path3.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2, config_path3];
        save(hc_dir.clone(), paths)?;

        // Clean specific sandboxes (index 1, which is sandbox2)
        remove(
            hc_dir.clone(),
            Existing {
                all: false,
                indices: vec![1],
            },
        )?;

        // Verify sandbox2 was removed but sandbox1 and sandbox3 still exist
        assert!(sandbox1.exists());
        assert!(!sandbox2.exists());
        assert!(sandbox3.exists());

        // .hc file must still exist
        let hc_file = hc_dir.join(".hc");
        assert!(hc_file.exists());

        let loaded_paths = load(hc_dir.clone())?;
        assert_eq!(loaded_paths.len(), 2);
        assert_eq!(loaded_paths[0], Ok(sandbox1.clone()));
        assert_eq!(loaded_paths[1], Ok(sandbox3.clone()));

        // Clean specific sandboxes (index 1, which is sandbox2)
        remove(
            hc_dir.clone(),
            Existing {
                all: false,
                indices: vec![0, 1],
            },
        )?;

        // Verify sandbox2 was removed but sandbox1 and sandbox3 still exist
        assert!(!sandbox1.exists());
        assert!(!sandbox3.exists());

        // .hc file must still exist
        let hc_file = hc_dir.join(".hc");
        assert!(!hc_file.exists());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_remove_all_sandboxes() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Remove all in empty folder
        remove(
            hc_dir.clone(),
            Existing {
                all: true,
                indices: vec![],
            },
        )?;
        // Verify the .hc file does not exist
        let hc_file = hc_dir.join(".hc");
        assert!(!hc_file.exists());

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Create a live lock file
        let live_file = hc_dir.join(".hc_live_0");
        fs::write(&live_file, "12345")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Clean all sandboxes (empty vector)
        remove(
            hc_dir.clone(),
            Existing {
                all: true,
                indices: vec![],
            },
        )?;

        // Verify all sandboxes were removed
        assert!(!sandbox1.exists());
        assert!(!sandbox2.exists());

        // Verify the .hc file was removed
        let hc_file = hc_dir.join(".hc");
        assert!(!hc_file.exists());

        // Verify the live lock file was removed
        assert!(!live_file.exists());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_remove_with_missing_directories() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        let hc_file = hc_dir.join(".hc");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        let sandbox3 = test_dir.join("sandbox3");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;
        fs::create_dir_all(&sandbox3)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        let config_path3 = ConfigRootPath::from(sandbox3.clone());
        let config_file_path3 = ConfigFilePath::from(config_path3.clone());
        fs::create_dir_all(config_file_path3.as_ref().parent().unwrap())?;
        fs::write(config_file_path3.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2, config_path3];
        save(hc_dir.clone(), paths)?;

        // Remove one of the directories manually
        fs::remove_dir_all(&sandbox1)?;

        // Remove missing sandbox
        remove(
            hc_dir.clone(),
            Existing {
                all: false,
                indices: vec![0],
            },
        )?;
        assert!(!sandbox1.exists());
        assert!(sandbox2.exists());
        assert!(sandbox3.exists());
        assert!(hc_file.exists());
        let loaded_paths = load(hc_dir.clone())?;
        assert_eq!(loaded_paths.len(), 2);
        assert_eq!(loaded_paths[0], Ok(sandbox2.clone()));
        assert_eq!(loaded_paths[1], Ok(sandbox3.clone()));

        // Remove one of the directories manually
        fs::remove_dir_all(&sandbox2)?;

        remove(
            hc_dir.clone(),
            Existing {
                all: false,
                indices: vec![0, 1],
            },
        )?;

        // Verify sandbox2 was removed (sandbox1 was already removed)
        assert!(!sandbox2.exists());
        assert!(!sandbox3.exists());
        // Verify the .hc file was removed (since all sandboxes were cleaned)
        assert!(!hc_file.exists());

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_remove_nonexistent_sandbox_index() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        let hc_file = hc_dir.join(".hc");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Try to clean a nonexistent sandbox index
        remove(
            hc_dir.clone(),
            Existing {
                all: false,
                indices: vec![2],
            },
        )?;
        // Verify sandboxes still exists
        assert!(sandbox1.exists());
        assert!(sandbox2.exists());
        // Verify the .hc file still exists
        assert!(hc_file.exists());

        // Try to clean a valid and nonexistent sandbox index
        remove(
            hc_dir.clone(),
            Existing {
                all: false,
                indices: vec![0, 2],
            },
        )?;
        // Verify sandboxes still exists
        assert!(!sandbox1.exists());
        assert!(sandbox2.exists());
        // Verify the .hc file still exists
        assert!(hc_file.exists());

        // Load the paths
        let loaded_paths = load(hc_dir.clone())?;
        // Verify the loaded paths
        assert_eq!(loaded_paths.len(), 1);
        assert_eq!(loaded_paths[0], Ok(sandbox2.clone()));

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    // Note: The list function primarily uses the load function and then formats the output
    // for display. Since we've already thoroughly tested the load function, we'll focus
    // on testing that the list function doesn't error with various inputs.

    #[test]
    fn test_list_with_valid_paths() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Call list with verbose=false (just testing that it doesn't error)
        list(hc_dir.clone(), false)?;

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_list_with_verbose() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        fs::create_dir_all(&sandbox1)?;

        // Create a valid conductor config file
        let config_root_path = ConfigRootPath::from(sandbox1.clone());
        let config =
            holochain_conductor_config::config::create_config(config_root_path.clone(), None)?;
        holochain_conductor_config::config::write_config(config_root_path.clone(), &config)?;

        // Save the path
        let paths = vec![config_root_path];
        save(hc_dir.clone(), paths)?;

        // Call list with verbose=true (just testing that it doesn't error)
        list(hc_dir.clone(), true)?;

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_list_with_missing_paths() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Create test sandbox directories
        let sandbox1 = test_dir.join("sandbox1");
        let sandbox2 = test_dir.join("sandbox2");
        fs::create_dir_all(&sandbox1)?;
        fs::create_dir_all(&sandbox2)?;

        // Create conductor config files
        let config_path1 = ConfigRootPath::from(sandbox1.clone());
        let config_file_path1 = ConfigFilePath::from(config_path1.clone());
        fs::create_dir_all(config_file_path1.as_ref().parent().unwrap())?;
        fs::write(config_file_path1.as_ref(), "dummy config")?;

        let config_path2 = ConfigRootPath::from(sandbox2.clone());
        let config_file_path2 = ConfigFilePath::from(config_path2.clone());
        fs::create_dir_all(config_file_path2.as_ref().parent().unwrap())?;
        fs::write(config_file_path2.as_ref(), "dummy config")?;

        // Save the paths
        let paths = vec![config_path1, config_path2];
        save(hc_dir.clone(), paths)?;

        // Remove one of the directories
        fs::remove_dir_all(&sandbox2)?;

        // Call list (just testing that it doesn't error when a path is missing)
        list(hc_dir.clone(), false)?;

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }

    #[test]
    fn test_list_empty_directory() -> anyhow::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempfile::tempdir()?;
        let test_dir = temp_dir.path();
        let hc_dir = test_dir.join("hc_dir");
        fs::create_dir_all(&hc_dir)?;

        // Call list on an empty directory (no .hc file)
        list(hc_dir.clone(), false)?;

        // No need for explicit cleanup, TempDir will handle it

        Ok(())
    }
}
