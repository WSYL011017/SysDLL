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

/// Run a scan. Returns the final report plus a progress receiver.
///
/// `targets` is the list of directories to walk. Non-recursive by default; the
/// caller controls via `ScanTarget::recursive` because we may want to scan just the
/// top of `System32` for quick checks.
pub fn run_scan(targets: Vec<ScanTarget>) -> ScanHandle {
    let started = std::time::Instant::now();

    // First pass: enumerate every candidate path. This is cheap and lets us show a
    // determinate progress bar.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for target in &targets {
        if !target.path.exists() {
            continue;
        }
        let walker = if target.recursive {
            WalkDir::new(&target.path)
        } else {
            WalkDir::new(&target.path).max_depth(1)
        };
        for entry in walker.follow_links(false).into_iter().flatten() {
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
            let _ = tx.send(ScanProgress {
                scanned: n,
                total,
                last_path: Some(path.clone()),
            });
            result
        })
        .collect();

    let total_files = files.len() as u64;
    let parsed_files = files.iter().filter(|f| f.pe.is_some()).count() as u64;
    let failed_files = files.iter().filter(|f| f.error.is_some()).count() as u64;

    let report = ScanReport {
        targets,
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
}
