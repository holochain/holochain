use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::builtin_commands;

/// List all runnable commands.
pub fn list_external_subcommands() -> Vec<String> {
    let prefix = "hc-";
    let suffix = env::consts::EXE_SUFFIX;
    let mut commands = Vec::new();

    let builtin_cmds = builtin_commands();

    for dir in search_directories() {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            _ => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(filename) => filename,
                _ => continue,
            };
            if !filename.starts_with(prefix) || !filename.ends_with(suffix) {
                continue;
            }
            let end = filename.len() - suffix.len();
            let subcommand = filename[prefix.len()..end].to_string();
            if is_executable(entry.path())
                && !builtin_cmds.contains(&filename.to_string())
                && !commands.contains(&subcommand)
            {
                commands.push(subcommand);
            }
        }
    }

    commands
}
#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::prelude::*;
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
#[cfg(windows)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_file()
}

fn search_directories() -> Vec<PathBuf> {
    let path_dirs = if let Some(val) = env::var_os("PATH") {
        env::split_paths(&val).collect()
    } else {
        vec![]
    };

    path_dirs
}
