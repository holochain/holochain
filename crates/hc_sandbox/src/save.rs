//! # Manage persistence of sandboxes
//!
//! This module gives basic helpers to save / load your sandboxes
//! to / from a `.hc` file.

use crate::cmds::Existing;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_conductor_config::config::read_config;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Representation of a loaded `.hc` file
#[derive(Debug)]
pub struct HcFile {
    /// Path to the directory containing the `.hc` file.
    pub dir: PathBuf,
    /// Vec of results after trying to read each path in the `.hc` file.
    pub existing_all: Vec<Result<ConfigRootPath, ConfigRootPath>>,
}

impl HcFile {
    /// Return only valid existing paths
    pub fn valid_paths(&self) -> Vec<ConfigRootPath> {
        self.existing_all.iter().flatten().cloned().collect()
    }

    /// Return only invalid existing paths
    pub fn invalid_paths(&self) -> Vec<ConfigRootPath> {
        self.existing_all
            .iter()
            .filter_map(|result| result.clone().err())
            .collect()
    }

    /// Return all paths
    pub fn all_paths(&self) -> Vec<ConfigRootPath> {
        self.existing_all
            .iter()
            .map(|res| match res {
                Ok(path) => path.clone(),
                Err(path) => path.clone(),
            })
            .collect()
    }

    /// Create a new `.hc` file on disk and return a newly constructed HcFile.
    /// All provided paths will be written in the file regardless of validity.
    /// Any pre-existing `.hc` file on disk will be overwritten.
    pub fn create(dir: PathBuf, paths: Vec<ConfigRootPath>) -> std::io::Result<Self> {
        // Load each given path
        let mut existing_all = Vec::new();
        for path in paths {
            if is_sandbox_path_valid(&path) {
                existing_all.push(Ok(path));
            } else {
                existing_all.push(Err(path));
            }
        }

        let new = Self { dir, existing_all };
        new.save()?;
        Ok(new)
    }

    /// Attempt to read the `.hc` file from disk,
    /// and try to load each sandbox path present in the file.
    /// Returns a HcFile according to results.
    pub fn load(hc_dir: PathBuf) -> std::io::Result<Self> {
        let hc_file = hc_dir.join(".hc");
        // If file does not exist, return empty struct
        if !hc_file.exists() {
            return Ok(Self {
                dir: hc_dir,
                existing_all: Vec::new(),
            });
        }
        if !hc_file.is_file() {
            return Err(std::io::Error::other(format!(
                "Failed to load hc file in {}",
                hc_dir.display()
            )));
        };
        let existing = std::fs::read_to_string(&hc_file).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "Failed to read config file '{}': {}",
                    hc_file.display(),
                    err
                ),
            )
        })?;

        let mut paths = Vec::new();
        for sandbox in existing.lines() {
            let path = ConfigRootPath::from(PathBuf::from(sandbox));
            if is_sandbox_path_valid(&path) {
                paths.push(Ok(path));
            } else {
                paths.push(Err(path));
            }
        }

        Ok(Self {
            dir: hc_dir,
            existing_all: paths,
        })
    }

    /// Overwrite `.hc` file on disk with all paths currently held by the object.
    fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "Failed to create directory: '{}': {}",
                    self.dir.display(),
                    err
                ),
            )
        })?;
        let hc_file = self.dir.join(".hc");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(hc_file)?;
        for path in self.existing_all.iter() {
            match path {
                Ok(path) => writeln!(file, "{}", path.display())?,
                Err(path) => writeln!(file, "{}", path.display())?,
            }
        }

        Ok(())
    }

    /// Append paths to HcFile and save to disk.
    pub fn append(&mut self, paths: Vec<ConfigRootPath>) -> std::io::Result<()> {
        for path in paths.into_iter() {
            if is_sandbox_path_valid(&path) {
                self.existing_all.push(Ok(path));
            } else {
                self.existing_all.push(Err(path));
            }
        }
        self.save()
    }

    /// Remove paths by their index in the `.hc` file
    /// and attempt to delete the sandbox folder.
    /// If no indices are passed in then they will all be removed.
    /// If no sandbox paths remain then all `.hc_*` files will be removed.
    pub fn remove(mut self, existing: Existing) -> std::io::Result<usize> {
        let cur_size = self.existing_all.len();
        let mut to_remove = Vec::new();
        let mut remaining = Vec::new();
        if existing.all {
            to_remove = self.existing_all.iter().collect();
        } else {
            // Tell user if index is out of range
            existing.indices.iter().for_each(|i| {
                if i >= &cur_size {
                    msg!("Warning: Provided index is out of range: {}", i)
                }
            });
            // split the to_be_removed from the remaining
            let index_set: std::collections::HashSet<usize> =
                existing.indices.iter().copied().collect();
            for (i, item) in self.existing_all.iter().enumerate() {
                if index_set.contains(&i) {
                    to_remove.push(item);
                } else {
                    remaining.push(item);
                }
            }
        }
        // Remove each requested sandbox dir
        for (i, maybe_path) in to_remove.into_iter().enumerate() {
            match maybe_path {
                Err(p) => msg!(
                    "Warning: Failed to delete sandbox at {}: {}",
                    i,
                    p.display()
                ),
                Ok(p) => {
                    if let Err(e) = std::fs::remove_dir_all(p.as_ref()) {
                        msg!(
                            "Warning: Failed to delete sandbox at {}: {}\nReason: {:?}",
                            i,
                            p.display(),
                            e
                        );
                    }
                }
            }
        }
        // Erase all other .hc* files
        if remaining.is_empty() {
            for entry in std::fs::read_dir(&self.dir).map_err(|err| {
                std::io::Error::new(
                    err.kind(),
                    format!(
                        "Failed to read directory: {}\nReason: {}",
                        self.dir.display(),
                        err
                    ),
                )
            })? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    if let Some(s) = entry.file_name().to_str() {
                        if s.starts_with(".hc_live_") {
                            std::fs::remove_file(entry.path()).map_err(|err| {
                                std::io::Error::new(
                                    err.kind(),
                                    format!("Failed to remove live lock at {}\nReason: {}", s, err),
                                )
                            })?
                        }
                    }
                }
            }
            let hc_auth = self.dir.join(".hc_auth");
            if hc_auth.exists() {
                std::fs::remove_file(&hc_auth).map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!(
                            "Failed to remove .hc_auth at {}\nReason: {}",
                            self.dir.display(),
                            err
                        ),
                    )
                })?;
            }
        }
        // Write new .hc file
        let remaining_as_paths = remaining
            .iter()
            .map(|res| match res {
                Ok(path) => path.clone(),
                Err(path) => path.clone(),
            })
            .collect();
        self = HcFile::create(self.dir, remaining_as_paths)?;
        // Return number of removed paths
        Ok(cur_size - self.existing_all.len())
    }

    /// Print out the sandboxes contained in the `.hc` file.
    pub fn list(&self, verbose: bool) -> std::io::Result<()> {
        if self.existing_all.is_empty() {
            msg!(
                "No sandboxes contained in {}",
                self.dir.join(".hc").display()
            );
            return Ok(());
        }
        msg!("Sandboxes contained in {}", self.dir.join(".hc").display());
        for (i, path) in self.existing_all.iter().enumerate() {
            let Ok(p) = path else {
                msg!(
                    "{}: {} -- UNAVAILABLE\n",
                    i,
                    path.as_ref().err().unwrap().display()
                );
                continue;
            };
            msg!("{}: {}\n", i, p.display());
            if verbose {
                let config =
                    holochain_conductor_config::config::read_config(p.clone()).map_err(|err| {
                        std::io::Error::other(format!(
                            "Failed to read config at {}: {}\nReason: {}",
                            i,
                            p.display(),
                            err
                        ))
                    })?;
                msg!("Conductor Config:\n{:?}\n", config);
            }
        }
        Ok(())
    }

    /// Lock this setup as running live and advertise the port.
    pub async fn lock_live(&self, path: &Path, port: u16) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|err| {
            std::io::Error::new(
                err.kind(),
                format!(
                    "Failed to create directory: '{}': {}",
                    self.dir.display(),
                    err
                ),
            )
        })?;
        let index = match self
            .valid_paths()
            .into_iter()
            .enumerate()
            .find(|p| p.1.as_ref() == path)
        {
            Some((i, _)) => i,
            None => return Ok(()),
        };
        let hc_live = self.dir.join(format!(".hc_live_{}", index));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(hc_live)
        {
            Ok(mut file) => {
                writeln!(file, "{}", port)?;
                let mut lock = get_file_locks().lock().await;
                lock.push(index);
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => {}
                _ => return Err(e),
            },
        }

        Ok(())
    }

    /// For each registered setup, if it has a lockfile, return the port of the running conductor,
    /// otherwise return None.
    /// The resulting Vec has the same number of elements as lines in the `.hc` file.
    pub fn load_ports(&self) -> std::io::Result<Vec<Option<u16>>> {
        let mut ports = Vec::new();
        for (i, _) in self.existing_all.iter().enumerate() {
            let hc_live = self.dir.join(format!(".hc_live_{}", i));
            if hc_live.exists() {
                let live = std::fs::read_to_string(hc_live.clone()).map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!("Failed to read file at '{}': {}", hc_live.display(), err),
                    )
                })?;
                let p = live.lines().next().and_then(|l| l.parse::<u16>().ok());
                ports.push(p)
            } else {
                ports.push(None);
            }
        }
        Ok(ports)
    }

    /// Same as load_ports but only returns ports for paths passed in.
    pub fn find_ports(&self, paths: &[ConfigRootPath]) -> std::io::Result<Vec<Option<u16>>> {
        let mut ports = Vec::new();
        let all_paths = self.existing_all.iter().flatten().collect::<Vec<_>>();
        for path in paths {
            let index = all_paths.iter().position(|p| *p == path);
            match index {
                Some(i) => {
                    let hc_live = self.dir.join(format!(".hc_live_{}", i));
                    if hc_live.exists() {
                        let live = std::fs::read_to_string(hc_live.clone()).map_err(|err| {
                            std::io::Error::new(
                                err.kind(),
                                format!("Failed to read file at '{}': {}", hc_live.display(), err),
                            )
                        })?;
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
    pub async fn release_ports(&self) -> std::io::Result<()> {
        let files = get_file_locks().lock().await;
        for file in files.iter() {
            let hc_live = self.dir.join(format!(".hc_live_{}", file));
            if hc_live.exists() {
                std::fs::remove_file(hc_live.clone()).map_err(|err| {
                    std::io::Error::new(
                        err.kind(),
                        format!("Failed to remove file at '{}': {}", hc_live.display(), err),
                    )
                })?;
            }
        }
        Ok(())
    }

    /// List the admin ports for each sandbox.
    pub async fn get_admin_ports(&self, paths: Vec<ConfigRootPath>) -> anyhow::Result<Vec<u16>> {
        let live_ports = self.find_ports(&paths[..])?;
        let mut ports = Vec::new();
        for (p, port) in paths.into_iter().zip(live_ports) {
            if let Some(port) = port {
                ports.push(port);
                continue;
            }
            if let Some(config) = read_config(p)? {
                if let Some(ai) = config.admin_interfaces {
                    if let Some(AdminInterfaceConfig {
                        driver: InterfaceDriver::Websocket { port, .. },
                    }) = ai.first()
                    {
                        ports.push(*port)
                    }
                }
            }
        }
        Ok(ports)
    }
}

fn get_file_locks() -> &'static tokio::sync::Mutex<Vec<usize>> {
    static FILE_LOCKS: OnceLock<tokio::sync::Mutex<Vec<usize>>> = OnceLock::new();

    FILE_LOCKS.get_or_init(|| tokio::sync::Mutex::new(Vec::new()))
}

/// Checks if a path points to an existing directory on disk for which we have permissions.
fn is_sandbox_path_valid(path: &ConfigRootPath) -> bool {
    path.exists() && path.is_dir()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Create a temporary folder to hold .hc files and subfolders.
    /// Create the .hc file and write the paths to each provided subfolder.
    /// Create subfolders on disk for those are in an Ok() wrapper.
    /// Returns the HcFile of that temporary folder.
    pub fn create_test_folder(subfolders: Vec<Result<&str, &str>>) -> HcFile {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(&temp_dir).unwrap();

        let hc_file = temp_dir.path().join(".hc");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(hc_file)
            .unwrap();
        for path in subfolders.into_iter() {
            match path {
                Ok(folder) => {
                    let path = temp_dir.path().join(folder);
                    writeln!(file, "{}", path.display()).unwrap();
                    std::fs::create_dir_all(&path).unwrap();
                }
                Err(folder) => {
                    let path = temp_dir.path().join(folder);
                    writeln!(file, "{}", path.display()).unwrap();
                }
            }
        }
        HcFile::load(PathBuf::from(temp_dir.path())).unwrap()
    }
    #[test]
    fn hc_file() {
        let hc_file = create_test_folder(vec![
            Ok(".ok1"),
            Err(".err1"),
            Ok(".ok2"),
            Ok(".ok3"),
            Err(".err2"),
        ]);

        assert_eq!(hc_file.all_paths().len(), 5);
        assert_eq!(hc_file.valid_paths().len(), 3);
        assert_eq!(hc_file.invalid_paths().len(), 2);
        assert_eq!(hc_file.invalid_paths()[0], hc_file.dir.join(".err1").into());
        assert_eq!(hc_file.valid_paths()[0], hc_file.dir.join(".ok1").into());
    }
}
