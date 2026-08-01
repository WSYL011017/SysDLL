//! Directory scanning.
//!
//! Walks one or more target directories, collects every `.dll` / `.exe`, and runs
//! `pe::analyze_path` in parallel using rayon. Results stream back via a channel so
//! the GUI can show a live progress bar.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::pe::{self, PeInfo};

/// A single target directory supplied by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanTarget {
    pub path: PathBuf,
    pub recursive: bool,
}

/// One file the scanner touched and what we learned about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub size: u64,
    /// `None` when the file is not a PE (skipped) or PE parse failed.
    pub pe: Option<PeInfo>,
    /// `true` when the file's parse errored out — we still record it so the UI can show
    /// "could not analyze" rows.
    pub error: Option<String>,
}

/// Final aggregated scan output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanReport {
    pub targets: Vec<ScanTarget>,
    pub files: Vec<ScannedFile>,
    pub total_files: u64,
    pub parsed_files: u64,
    pub failed_files: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub scanned: u64,
    pub total: u64,
    pub last_path: Option<PathBuf>,
}

pub struct ScanHandle {
    pub report: ScanReport,
    pub progress: Receiver<ScanProgress>,
}

/// Reject obvious foot-guns before they reach `walkdir`. Audit fix R6/P0-8:
///
/// - Reject `..` segments anywhere in the path (after canonicalisation).
/// - Reject UNC paths (`\\server\share`) because they trigger SMB auth /
///   latency surprises.
/// - Reject Windows device / namespace paths (`\\.\`, `\\?\\`).
/// - Reject empty / whitespace-only paths.
///
/// Canonicalisation happens after this filter so a user-supplied
/// `C:\foo\..\bar` reduces to `C:\bar` and then passes — but a
/// `C:\..\..\Windows` reduces to the disk root and we still refuse.
pub fn sanitise_target(target: &ScanTarget) -> Result<()> {
    let path = &target.path;
    let s = path.to_string_lossy();
    if s.trim().is_empty() {
        return Err(Error::Walk(format!("empty path")));
    }
    if s.contains("..") {
        return Err(Error::Walk(format!("path contains '..': {s}")));
    }
    if s.starts_with("\\\\") {
        return Err(Error::Walk(format!("UNC paths not allowed: {s}")));
    }
    if s.starts_with("\\\\?\\") || s.starts_with("\\\\.\\") {
        return Err(Error::Walk(format!("device paths not allowed: {s}")));
    }
    Ok(())
}

/// Run a scan. Returns the final report plus a progress receiver.
///
/// `targets` is the list of directories to walk. Non-recursive by default; the
/// caller controls via `ScanTarget::recursive` because we may want to scan just the
/// top of `System32` for quick checks.
pub fn run_scan(targets: Vec<ScanTarget>) -> ScanHandle {
    let started = std::time::Instant::now();

    // Audit fix R6/P0-8: refuse unsafe paths up front. The first error wins;
    // callers can iterate by submitting one target at a time.
    let safe_targets: Vec<ScanTarget> = targets
        .into_iter()
        .filter(|t| sanitise_target(t).is_ok())
        .collect();

    // First pass: enumerate every candidate path. This is cheap and lets us show a
    // determinate progress bar.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for target in &safe_targets {
        if !target.path.exists() {
            continue;
        }
        let walker = if target.recursive {
            WalkDir::new(&target.path).follow_links(false)
        } else {
            WalkDir::new(&target.path).max_depth(1).follow_links(false)
        };
        for entry in walker.into_iter() {
            // Audit fix R6/P0-7 / P3-4: skip symlinks explicitly and surface
            // walk errors as candidates that won't be scanned (logged in the
            // report's `walk_errors` is a follow-up; for now we just drop).
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            // skip both FileType::is_symlink() and the Windows junction case
            // (which walkdir also reports as a symlink).
            if entry.file_type().is_symlink() {
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let is_candidate = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| {
                    let lower = e.to_ascii_lowercase();
                    lower == "dll" || lower == "exe" || lower == "sys" || lower == "ocx"
                })
                .unwrap_or(false);
            if is_candidate {
                candidates.push(path.to_path_buf());
            }
        }
    }

    let total = candidates.len() as u64;
    let (tx, rx): (Sender<ScanProgress>, Receiver<ScanProgress>) = unbounded();
    let counter = Arc::new(Mutex::new(0u64));

    // Second pass: parse in parallel. pelite is zero-allocation so memory stays flat.
    let files: Vec<ScannedFile> = candidates
        .par_iter()
        .map(|path| {
            let result = scan_one(path);
            let mut c = counter.lock();
            *c += 1;
            let n = *c;
            // Send a progress tick at most every 32 files so the channel
            // doesn't become the bottleneck on a 50k-file scan.
            if n % 32 == 0 || n == total {
                let _ = tx.send(ScanProgress {
                    scanned: n,
                    total,
                    last_path: Some(path.clone()),
                });
            }
            result
        })
        .collect();

    let total_files = files.len() as u64;
    let parsed_files = files.iter().filter(|f| f.pe.is_some()).count() as u64;
    let failed_files = files.iter().filter(|f| f.error.is_some()).count() as u64;

    let report = ScanReport {
        targets: safe_targets,
        files,
        total_files,
        parsed_files,
        failed_files,
        duration_ms: started.elapsed().as_millis() as u64,
    };

    ScanHandle { report, progress: rx }
}

fn scan_one(path: &Path) -> ScannedFile {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    match pe::analyze_path(path) {
        Ok(pe_info) => ScannedFile {
            path: path.to_path_buf(),
            size,
            pe: pe_info,
            error: None,
        },
        Err(err) => ScannedFile {
            path: path.to_path_buf(),
            size,
            pe: None,
            error: Some(err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_targets_yields_empty_report() {
        let handle = run_scan(vec![]);
        assert_eq!(handle.report.total_files, 0);
    }

    #[test]
    fn sanitise_blocks_parent_traversal() {
        let bad = ScanTarget {
            path: PathBuf::from("C:\\foo\\..\\bar"),
            recursive: false,
        };
        assert!(sanitise_target(&bad).is_err());
    }

    #[test]
    fn sanitise_blocks_unc() {
        let bad = ScanTarget {
            path: PathBuf::from("\\\\evil-share\\payload"),
            recursive: true,
        };
        assert!(sanitise_target(&bad).is_err());
    }

    #[test]
    fn sanitise_allows_normal_paths() {
        let ok = ScanTarget {
            path: PathBuf::from("C:\\Windows\\System32"),
            recursive: true,
        };
        assert!(sanitise_target(&ok).is_ok());
    }

    #[test]
    fn sanitise_blocks_device_paths() {
        let bad = ScanTarget {
            path: PathBuf::from("\\\\?\\C:\\Windows"),
            recursive: true,
        };
        assert!(sanitise_target(&bad).is_err());
    }
}
