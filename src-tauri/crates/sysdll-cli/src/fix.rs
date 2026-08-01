//! Repair operations: copy a known-good DLL into the system tree.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::backup::backup_for;

/// Install `source` at `target`, backing up any existing file first.
///
/// Audit fix P3-2: returning `PathBuf::new()` for "no existing file" made
/// the front-end unable to tell "fixed without backup" from "fix failed
/// before any copy". We now return `Option<PathBuf>` so missing backups
/// stay distinguishable at the IPC boundary.
pub fn install(source: &Path, target: &Path) -> Result<Option<PathBuf>> {
    if !source.exists() {
        anyhow::bail!("source file does not exist: {}", source.display());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    if target.exists() {
        let backup = backup_for(target)?;
        fs::copy(target, &backup)
            .with_context(|| format!("backing up {} -> {}", target.display(), backup.display()))?;
        fs::copy(source, target)
            .with_context(|| format!("installing {} -> {}", source.display(), target.display()))?;
        Ok(Some(backup))
    } else {
        fs::copy(source, target)
            .with_context(|| format!("installing {} -> {}", source.display(), target.display()))?;
        Ok(None)
    }
}
