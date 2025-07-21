//! # Manage persistence of sandboxes
//!
//! This module gives basic helpers to save / load your sandboxes
//! to / from a `.hc` file.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::io::Write;

use anyhow::{anyhow, Context};
use holochain_conductor_api::conductor::paths::{ConfigFilePath, ConfigRootPath};


/// Representation of a loaded `.hc` file
#[derive(Debug)]
pub struct HcFile {
    /// Path to the `.hc` file.
    pub dir: PathBuf,
    /// Vec of results after trying to read each path in the `.hc` file.
    pub existing_all: Vec<anyhow::Result<ConfigRootPath>>,
}

impl HcFile {
    /// Default Constructor
    pub fn new(dir: PathBuf, paths: Vec<ConfigRootPath>) -> Self {
        // Load each given path
        let mut existing_all = Vec::new();
        for path in paths {
            if path.exists() && path.is_dir() {
                existing_all.push(Ok(path));
            } else {
                existing_all.push(Err(anyhow!("{}", path.display())));
            }
        }
        //
        Self {
            dir,
            existing_all,
        }
    }
    
    /// Return only valid existing paths
    pub fn existing_valids(&self) -> Vec<ConfigRootPath> { self.existing_all.iter().flatten().cloned().collect() }

    /// Try to read the `.hc` file from disk,
    /// try to load each sandbox path
    /// and return a HcFile according to results.
    pub fn try_load(hc_dir: PathBuf) -> anyhow::Result<Self> {
        let hc_file = hc_dir.join(".hc");
        dbg!(&hc_file);
        if !hc_file.exists() {
            return Ok(Self {
                dir: hc_dir,
                existing_all: Vec::new(),
            });
        }
        if !hc_file.is_file() {
            return Err(anyhow!("Failed to load hc file in {}", hc_dir.display()));
        };
        let existing = std::fs::read_to_string(&hc_file)
            .with_context(|| format!("Failed to read file: {}", hc_file.display()))?;


        let mut paths = Vec::new();
        for sandbox in existing.lines() {
            let path = ConfigRootPath::from(PathBuf::from(sandbox));
            let config_file_path = ConfigFilePath::from(path.clone());
            if config_file_path.as_ref().exists() && config_file_path.as_ref().is_dir() {
                paths.push(Ok(path));
            } else {
                paths.push(Err(anyhow!("{}", path.display())));
            }
        }


        Ok(Self {
            dir: hc_dir,
            existing_all: paths,
        })
    }

    /// Overwrite `.hc` file on disk with the valid content currently in this object
    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create directory: {}", self.dir.display()))?;
        let hc_file = self.dir.join(".hc");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(hc_file)?;
        for path in self.existing_valids().iter() {
            writeln!(file, "{}", path.display())?;
        }
        Ok(())
    }

    /// Append paths to HcFile and save to disk
    pub fn append(&mut self, paths: Vec<ConfigRootPath>) -> anyhow::Result<()> {
        for path in paths.into_iter() {
            if path.exists() && path.is_dir() {
                self.existing_all.push(Ok(path));
            } else {
                self.existing_all.push(Err(anyhow!("{}", path.display())));
            }
        }
        return self.save();
    }

    /// Remove paths by their index in the `.hc` file.
    /// If no indices are passed in then they will all be deleted.
    /// If no sandbox remains then all `.hc_*` files will be removed.
    pub fn remove(mut self, indices_to_remove: Vec<usize>) -> anyhow::Result<usize> {
        let cur_size = self.existing_all.len();
        let mut to_remove = Vec::new();
        let mut remaining = Vec::new();
        if indices_to_remove.is_empty() {
            to_remove = self.existing_all.iter().collect();
        } else {
            // split the to_be_removed from the remaining
            let index_set: std::collections::HashSet<usize> = indices_to_remove.iter().copied().collect();
            for (i, item) in self.existing_all.iter().enumerate() {
                if index_set.contains(&i) {
                    to_remove.push(item);
                } else {
                    remaining.push(item);
                }
            }
        }
        // Remove each requested sandbox dir
        for maybe_path in to_remove.into_iter() {
            match maybe_path {
                Err(e) => msg!("Warning: Failed to delete sandbox at \"{}\".", e),
                Ok(p) => {
                    if let Err(e) = std::fs::remove_dir_all(p.as_ref()) {
                        msg!("Warning: Failed to delete sandbox at \"{}\" because {:?}", p.display(), e);
                    }
                },
            }
        }
        // // Erase `.hc` file
        // let hc_file = self.dir.join(".hc");
        // if hc_file.exists() {
        //     std::fs::remove_file(&hc_file)
        //         .with_context(|| format!("Failed to remove .hc file at {}", self.dir.display()))?;
        // }
        //
        let valid_remaining: Vec<ConfigRootPath> = remaining.into_iter().flatten().cloned().collect();
        // Erase all other files
        if valid_remaining.is_empty() {
            for entry in std::fs::read_dir(&self.dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    if let Some(s) = entry.file_name().to_str() {
                        if s.starts_with(".hc_live_") {
                            std::fs::remove_file(entry.path())
                                .with_context(|| format!("Failed to remove live lock at {}", s))?;
                        }
                    }
                }
            }
            let hc_auth = self.dir.join(".hc_auth");
            if hc_auth.exists() {
                std::fs::remove_file(&hc_auth).with_context(|| {
                    format!("Failed to remove .hc_auth file at {}", self.dir.display())
                })?;
            }
        }
        // Write new .hc file
        self = HcFile::new(self.dir, valid_remaining);
        self.save()?;
        //
        Ok(cur_size - self.existing_all.len())
    }


    /// Print out the sandboxes contained in the `.hc` file.
    pub fn list(&self, verbose: bool) -> anyhow::Result<()> {
        if self.existing_all.is_empty() {
            msg!("No sandboxes contained in `{}`", self.dir.join(".hc").display());
            return Ok(());
        }
        msg!("Sandboxes contained in `{}`", self.dir.join(".hc").display());
        for (i, path) in self.existing_all.iter().enumerate() {
            let Ok(p) = path else {
                msg!("{}: {} -- UNAVAILABLE\n", i, path.as_ref().err().unwrap().to_string());
                continue;
            };
            msg!("{}: {}\n", i, p.display());
            if verbose {
                let config = holochain_conductor_config::config::read_config(
                    ConfigRootPath::from(p.clone()),
                )?;
                msg!("Conductor Config:\n{:?}\n",config);
            }
        }
        Ok(())
    }

    /// Lock this setup as running live and advertise the port.
    pub async fn lock_live(&self, path: &Path, port: u16) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create directory: {}", self.dir.display()))?;
        let index = match self.existing_valids().into_iter().enumerate().find(|p| p.1.as_ref() == path) {
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
                _ => return Err(e.into()),
            },
        }

        Ok(())
    }

    /// For each registered setup, if it has a lockfile, return the port of the running conductor,
    /// otherwise return None.
    /// The resulting Vec has the same number of elements as lines in the `.hc` file.
    pub fn load_ports(&self) -> anyhow::Result<Vec<Option<u16>>> {
        let mut ports = Vec::new();
        for (i, _) in self.existing_all.iter().enumerate() {
            let hc_live = self.dir.join(format!(".hc_live_{}", i));
            if hc_live.exists() {
                let live = std::fs::read_to_string(hc_live)?;
                let p = live.lines().next().and_then(|l| l.parse::<u16>().ok());
                ports.push(p)
            } else {
                ports.push(None);
            }
        }
        Ok(ports)
    }

    /// Same as load_ports but only returns ports for paths passed in.
    pub fn find_ports(&self, paths: &[ConfigRootPath]) -> anyhow::Result<Vec<Option<u16>>> {
        let mut ports = Vec::new();
        let all_paths = self.existing_all.iter().flatten().collect::<Vec<_>>();
        for path in paths {
            let index = all_paths.iter().position(|p| *p == path);
            match index {
                Some(i) => {
                    let hc_live = self.dir.join(format!(".hc_live_{}", i));
                    if hc_live.exists() {
                        let live = std::fs::read_to_string(hc_live)?;
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
    pub async fn release_ports(&self) -> anyhow::Result<()> {
        let files = get_file_locks().lock().await;
        for file in files.iter() {
            let hc_live = self.dir.join(format!(".hc_live_{}", file));
            if hc_live.exists() {
                std::fs::remove_file(hc_live)?;
            }
        }
        Ok(())
    }
}


fn get_file_locks() -> &'static tokio::sync::Mutex<Vec<usize>> {
    static FILE_LOCKS: OnceLock<tokio::sync::Mutex<Vec<usize>>> = OnceLock::new();

    FILE_LOCKS.get_or_init(|| tokio::sync::Mutex::new(Vec::new()))
}
