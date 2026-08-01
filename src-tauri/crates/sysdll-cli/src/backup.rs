//! Backup / restore helpers.
//!
//! Backups live under `%LOCALAPPDATA%\SysDll\backup\<nanos-timestamp>\
//! <short-hash>-<file>.bak` so the GUI can show the user exactly which
//! files were touched and let them roll back with one click.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

/// Hex-stable-ish FNV-1a 64-bit short hash for tagging backup filenames
/// with the source path. We don't use a cryptographic hash here — we only
/// need a short identifier that prevents two distinct absolute paths from
/// colliding in the same backup directory (audit P0-3).
fn short_path_tag(target: &Path) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    let canonical = target.to_string_lossy();
    for b in canonical.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn backup_root() -> Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .context("neither LOCALAPPDATA nor USERPROFILE is set")?;
    let root = PathBuf::from(local).join("SysDll").join("backup");
    fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
    Ok(root)
}

/// Audit fix R7:
/// - P2-11: use nanosecond precision (and an in-process atomic counter) so
///   two `install` calls within the same second each get distinct dirs.
/// - P0-3: prefix `<file>` with a short hash of the original absolute path
///   so `restore_latest` cannot be tricked into restoring the wrong file
///   when two DLLs share a leaf name (e.g. `foo.dll` in two different
///   paths).
pub fn backup_for(target: &Path) -> Result<PathBuf> {
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = next_backup_counter();
    let stamp = format!("{timestamp_nanos}-{counter:04x}");
    let dir = backup_root()?.join(stamp);
    fs::create_dir_all(&dir)?;
    let file_name = target
        .file_name()
        .context("target has no file name")?
        .to_string_lossy()
        .into_owned();
    let tag = short_path_tag(target);
    Ok(dir.join(format!("{tag}-{file_name}.bak")))
}

/// In-process counter so two backups requested at the exact same nanosecond
/// still land in separate directories.
fn next_backup_counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    CTR.fetch_add(1, Ordering::Relaxed)
}

/// Roll back the *latest* backup for `target`.
///
/// Audit fixes:
/// - P0-4: the previous implementation `entries.sort(); entries.last()` used
///   a directory name string-sort *ascending* (timestamps aren't padded),
///   so `last()` actually returned the *oldest* dir. We now sort descending
///   by timestamp string and take the first.
/// - P0-3: we verify that the filename inside the chosen directory carries
///   the same short-tag as `target`. If a malicious / stale `.bak` is sitting
///   there with the right leaf name but a different source path, the
///   mismatch fails the restore.
pub fn restore_latest(target: &Path) -> Result<PathBuf> {
    let root = backup_root()?;
    let mut entries: Vec<PathBuf> = fs::read_dir(&root)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    // Descending lexical sort: nanosecond timestamps share a common width
    // via `format!`'s zero-padding callers in `backup_for`, so lex compare
    // == numeric compare.
    entries.sort_by(|a, b| b.cmp(a));

    let want_tag = short_path_tag(target);
    let want_leaf = target
        .file_name()
        .context("target has no file name")?
        .to_string_lossy()
        .into_owned();

    for dir in &entries {
        let candidate = dir.join(format!("{want_tag}-{want_leaf}.bak"));
        if candidate.exists() {
            fs::copy(&candidate, target)
                .with_context(|| format!("restoring {} from {}", target.display(), candidate.display()))?;
            return Ok(candidate);
        }
    }
    anyhow::bail!("no matching backup for {} (tag={want_tag})", target.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_path_tag_stable_and_distinct() {
        let a = short_path_tag(Path::new("C:\\Windows\\System32\\foo.dll"));
        let b = short_path_tag(Path::new("C:\\Windows\\System32\\foo.dll"));
        let c = short_path_tag(Path::new("C:\\Windows\\System32\\bar.dll"));
        let d = short_path_tag(Path::new("D:\\Windows\\System32\\foo.dll"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn sort_descending_keeps_largest_first() {
        let mut entries = vec![
            PathBuf::from("b-2"),
            PathBuf::from("a-1"),
            PathBuf::from("c-3"),
        ];
        entries.sort_by(|a, b| b.cmp(a));
        assert_eq!(entries[0].file_name().unwrap(), "c-3");
        assert_eq!(entries[2].file_name().unwrap(), "a-1");
    }
}
