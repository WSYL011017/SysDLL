//! sysdll-cli: the elevated child process invoked by the GUI when repairs need
//! to touch System32 / WinSxS / HKLM.
//!
//! Communication protocol with the parent (GUI) uses line-delimited JSON over
//! stdin / stdout. Every command emits progress events so the GUI can render a
//! live log without polling.

use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use sysdll_core::scan::{ScanReport, ScanTarget};

mod backup;
mod fix;
mod ipc;

use ipc::{Event, Request};

#[derive(Parser, Debug)]
#[command(
    name = "sysdll-cli",
    about = "SysDll elevated repair helper (requires Administrator)",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// When set, the CLI exchanges JSON-RPC messages on stdin/stdout instead of
    /// running a single subcommand and exiting. The GUI uses this mode.
    #[arg(long)]
    ipc: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Scan a directory and emit a JSON ScanReport on stdout.
    Scan {
        #[arg(long)]
        target: PathBuf,
        #[arg(long, default_value_t = true)]
        recursive: bool,
        #[arg(long)]
        json: bool,
    },
    /// Copy a DLL into the system tree, with automatic backup.
    Fix {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        source: PathBuf,
    },
    /// Roll back the most recent backup for a path.
    RestoreBackup { #[arg(long)] target: PathBuf },
    /// Print the path of the local backup directory.
    BackupDir,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.ipc {
        return match run_ipc() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("ipc loop terminated: {err}");
                ExitCode::FAILURE
            }
        };
    }
    match run_cli(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_cli(cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Scan { target, recursive, json } => {
            let report = run_scan(target, recursive)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Scanned {} files ({} parsed, {} failed) in {} ms",
                    report.total_files, report.parsed_files, report.failed_files, report.duration_ms
                );
            }
        }
        Command::Fix { target, source } => {
            let backup = fix::install(&source, &target)?;
            println!("Installed {} (backup: {})", target.display(), backup.display());
        }
        Command::RestoreBackup { target } => {
            let restored = backup::restore_latest(&target)?;
            println!("Restored from {}", restored.display());
        }
        Command::BackupDir => {
            println!("{}", backup::backup_root()?.display());
        }
    }
    Ok(())
}

fn run_scan(target: PathBuf, recursive: bool) -> anyhow::Result<ScanReport> {
    let targets = vec![ScanTarget { path: target, recursive }];
    let handle = sysdll_core::scan::run_scan(targets);
    Ok(handle.report)
}

fn run_ipc() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(err) => {
                ipc::send(&mut out, &Event::Error { message: err.to_string() })?;
                continue;
            }
        };
        match req {
            Request::Ping => {
                ipc::send(&mut out, &Event::Pong)?;
            }
            Request::Scan { target, recursive } => {
                ipc::send(&mut out, &Event::Log { level: "info".into(), message: format!("scanning {}", target.display()) })?;
                match run_scan(target, recursive) {
                    Ok(report) => ipc::send(&mut out, &Event::ScanDone { report })?,
                    Err(err) => ipc::send(&mut out, &Event::Error { message: err.to_string() })?,
                }
            }
            Request::Fix { target, source } => {
                ipc::send(&mut out, &Event::Log { level: "info".into(), message: format!("installing {} <- {}", target.display(), source.display()) })?;
                match fix::install(&source, &target) {
                    Ok(backup) => ipc::send(&mut out, &Event::FixDone {
                        target: target.display().to_string(),
                        backup: backup.display().to_string(),
                    })?,
                    Err(err) => ipc::send(&mut out, &Event::Error { message: err.to_string() })?,
                }
            }
            Request::RestoreBackup { target } => match backup::restore_latest(&target) {
                Ok(restored) => ipc::send(&mut out, &Event::RestoreDone {
                    target: target.display().to_string(),
                    backup: restored.display().to_string(),
                })?,
                Err(err) => ipc::send(&mut out, &Event::Error { message: err.to_string() })?,
            },
            Request::Shutdown => {
                ipc::send(&mut out, &Event::Log { level: "info".into(), message: "shutdown".into() })?;
                break;
            }
        }
    }
    Ok(())
}
