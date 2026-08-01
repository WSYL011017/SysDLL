//! JSON-RPC over stdin/stdout for the elevated child.

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use sysdll_core::scan::ScanReport;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Scan { target: PathBuf, recursive: bool },
    Fix { target: PathBuf, source: PathBuf },
    RestoreBackup { target: PathBuf },
    Shutdown,
}

/// All events the GUI may receive. Each variant mirrors a `CliEvent`
/// branch in `src/types/sysdll.ts`; a Rust variant without a TS counterpart
/// (or vice-versa) is a 1:1 contract violation that must be caught in CI.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Pong,
    Log { level: String, message: String },
    Error { message: String },
    ScanDone { report: ScanReport },
    /// Audit fix R6/P1-1: previously the scan's progress channel was
    /// discarded by the GUI; we now emit periodic progress ticks so the
    /// progress bar reflects real throughput.
    Progress { scanned: usize, total: usize, current: Option<String> },
    FixDone { target: String, backup: String },
    RestoreDone { target: String, backup: String },
    /// Audit fix R8/P1-9: emitted when the parent drops the stdin handle
    /// (or the IPC loop exits for any reason). GUI flips `cliRunning = false`.
    Exit { code: Option<i32> },
}

pub fn send<W: Write>(writer: &mut W, event: &Event) -> Result<()> {
    let line = serde_json::to_string(event)?;
    writeln!(writer, "{line}")?;
    writer.flush()?;
    Ok(())
}
