//! Repair operations: copy a known-good DLL into the system tree.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::backup::{backup_for, restore_latest};

/// Install `source` at `target`, backing up any existing file first.
/// Returns the path to the backup.
pub fn install(source: &Path, target: &Path) -> Result<std::path::PathBuf> {
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
        Ok(backup)
    } else {
        fs::copy(source, target)
            .with_context(|| format!("installing {} -> {}", source.display(), target.display()))?;
        Ok(std::path::PathBuf::new())
    }
}

#[allow(dead_code)]
fn _silence(_: &Path) -> Result<()> {
    let _ = restore_latest;
    Ok(())
}
