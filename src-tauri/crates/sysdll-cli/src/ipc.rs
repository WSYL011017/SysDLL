//! JSON-RPC over stdin/stdout for the elevated child.

use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use sysdll_core::scan::ScanReport;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Scan { target: std::path::PathBuf, recursive: bool },
    Fix { target: std::path::PathBuf, source: std::path::PathBuf },
    RestoreBackup { target: std::path::PathBuf },
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Pong,
    Log { level: String, message: String },
    Error { message: String },
    ScanDone { report: ScanReport },
    FixDone { target: String, backup: String },
    RestoreDone { target: String, backup: String },
}

pub fn send<W: Write>(writer: &mut W, event: &Event) -> Result<()> {
    let line = serde_json::to_string(event)?;
    writeln!(writer, "{line}")?;
    writer.flush()?;
    Ok(())
}
