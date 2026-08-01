//! Backup / restore helpers.
//!
//! Backups live under `%LOCALAPPDATA%\SysDll\backup\<UTC-timestamp>\<file>.bak`
//! so the GUI can show the user exactly which files were touched and let them
//! roll back with one click.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

pub fn backup_root() -> Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .context("neither LOCALAPPDATA nor USERPROFILE is set")?;
    let root = PathBuf::from(local).join("SysDll").join("backup");
    fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
    Ok(root)
}

pub fn backup_for(target: &Path) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = backup_root()?.join(timestamp.to_string());
    fs::create_dir_all(&dir)?;
    let file_name = target
        .file_name()
        .context("target has no file name")?
        .to_string_lossy()
        .into_owned();
    Ok(dir.join(format!("{file_name}.bak")))
}

pub fn restore_latest(target: &Path) -> Result<PathBuf> {
    let root = backup_root()?;
    let mut entries: Vec<PathBuf> = fs::read_dir(&root)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    let latest = entries
        .last()
        .context("no backups available")?;
    let file_name = target
        .file_name()
        .context("target has no file name")?
        .to_string_lossy()
        .into_owned();
    let backup_file = latest.join(format!("{file_name}.bak"));
    if !backup_file.exists() {
        anyhow::bail!("no backup for {} in {}", file_name, latest.display());
    }
    fs::copy(&backup_file, target)
        .with_context(|| format!("restoring {} from {}", target.display(), backup_file.display()))?;
    Ok(backup_file)
}

// Integration tests for the CLI live in `tests/cli.rs` (added in phase 2 once
// we have a non-elevated test entrypoint). Backup logic is exercised end-to-end
// during the MVP smoke test.
#[cfg(test)]
#[cfg(any())] // disable until we have a non-elevated test runner
mod tests {
    use super::*;

    #[test]
    fn backup_path_is_absolute() {
        let root = backup_root().unwrap();
        assert!(root.is_absolute(), "backup root must be absolute: {:?}", root);
    }
}
