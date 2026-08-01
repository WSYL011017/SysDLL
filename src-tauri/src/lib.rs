//! Tauri command surface for the SysDll GUI.
//!
//! The GUI process runs asInvoker (no UAC prompt). When repairs need admin
//! rights we `ShellExecuteW` the `sysdll-cli.exe` child with the `runas` verb;
//! see [`launch_cli`]. The child then streams JSON-RPC events back over its
//! stdout, which the GUI turns into Tauri events the frontend can listen for.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sysdll_core::diag::{run_diagnostics, Diagnostic};
use sysdll_core::graph::DependencyGraph;
use sysdll_core::scan::{ScanReport, ScanTarget};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .manage(cli::ChildState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            scan_targets,
            run_diagnostics_cmd,
            launch_cli,
            list_backups,
            restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Default)]
struct AppState {
    last_report: Mutex<Option<ScanReport>>,
    last_graph: Mutex<Option<DependencyGraph>>,
}

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

/// Run a scan on the requested targets. The result is cached so the
/// `run_diagnostics` command can re-derive issues without re-scanning.
#[tauri::command]
fn scan_targets(
    state: tauri::State<'_, AppState>,
    targets: Vec<ScanTarget>,
) -> Result<ScanReport, String> {
    let handle = sysdll_core::scan::run_scan(targets);
    let report = handle.report;
    let graph = DependencyGraph::from_scan(&report);
    *state.last_report.lock().unwrap() = Some(report.clone());
    *state.last_graph.lock().unwrap() = Some(graph);
    Ok(report)
}

#[tauri::command]
fn run_diagnostics_cmd(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Diagnostic>, String> {
    let report_guard = state.last_report.lock().unwrap();
    let graph_guard = state.last_graph.lock().unwrap();
    let report = report_guard.as_ref().ok_or("no scan has been run yet")?;
    let graph = graph_guard.as_ref().ok_or("no scan has been run yet")?;
    Ok(run_diagnostics(report, graph))
}

/// Launch the elevated CLI child. Returns the PID so the frontend can attach
/// to its stdout / stderr streams.
#[tauri::command]
fn launch_cli(app: tauri::AppHandle) -> Result<u32, String> {
    let pid = cli::launch(&app).map_err(|e| e.to_string())?;
    Ok(pid)
}

#[tauri::command]
fn list_backups() -> Result<Vec<BackupEntry>, String> {
    cli::list_backups().map_err(|e| e.to_string())
}

#[tauri::command]
fn restore_backup(target: PathBuf) -> Result<String, String> {
    cli::restore_backup(&target).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupEntry {
    pub timestamp: String,
    pub files: Vec<PathBuf>,
}

// CLI child plumbing lives in its own module so the IPC surface above stays compact.
mod cli {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::Mutex;

    use tauri::{AppHandle, Emitter, Manager};

    const TAURI_EVENT_CLI: &str = "sysdll://cli-event";

    pub fn launch(app: &AppHandle) -> anyhow::Result<u32> {
        let cli_path = locate_cli_binary(app)?;
        let mut child = Command::new(cli_path)
            .arg("--ipc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let pid = child.id();
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let stdin = child.stdin.take().expect("stdin piped");

        let app_handle = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(|l| l.ok()) {
                if line.trim().is_empty() {
                    continue;
                }
                let _ = app_handle.emit(TAURI_EVENT_CLI, line);
            }
        });

        let app_handle = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(|l| l.ok()) {
                if line.trim().is_empty() {
                    continue;
                }
                let _ = app_handle.emit(TAURI_EVENT_CLI, format!("{{\"event\":\"stderr\",\"line\":\"{line}\"}}"));
            }
        });

        // Stash the child so the GUI can drive it later.
        let state = app.state::<ChildState>();
        let mut guard = state.child.lock().unwrap();
        if let Some(mut prev) = guard.take() {
            let _ = prev.child.kill();
        }
        *guard = Some(ChildHandle { child, stdin: Mutex::new(stdin) });
        Ok(pid)
    }

    pub fn list_backups() -> anyhow::Result<Vec<BackupEntry>> {
        let root = backup_root()?;
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let files: Vec<PathBuf> = std::fs::read_dir(entry.path())?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .collect();
            out.push(BackupEntry {
                timestamp: entry.file_name().to_string_lossy().into_owned(),
                files,
            });
        }
        out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(out)
    }

    pub fn restore_backup(target: &Path) -> anyhow::Result<String> {
        let root = backup_root()?;
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        let latest = entries.last().ok_or_else(|| anyhow::anyhow!("no backups"))?;
        let name = target
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("target has no file name"))?
            .to_string_lossy()
            .into_owned();
        let backup = latest.join(format!("{name}.bak"));
        if !backup.exists() {
            anyhow::bail!("no backup for {name}");
        }
        std::fs::copy(&backup, target)?;
        Ok(backup.display().to_string())
    }

    fn locate_cli_binary(app: &AppHandle) -> anyhow::Result<PathBuf> {
        // In dev, cargo puts it at target/debug/sysdll-cli.exe next to the GUI binary.
        // In production, the bundler places it next to the main exe.
        let exe_dir = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("no parent for current exe"))?
            .to_path_buf();
        let candidate = exe_dir.join(if cfg!(windows) {
            "sysdll-cli.exe"
        } else {
            "sysdll-cli"
        });
        if candidate.exists() {
            return Ok(candidate);
        }
        // Fallback to the resource directory baked by the bundler.
        if let Ok(resource) = app.path().resource_dir() {
            let alt = resource.join(if cfg!(windows) {
                "sysdll-cli.exe"
            } else {
                "sysdll-cli"
            });
            if alt.exists() {
                return Ok(alt);
            }
        }
        anyhow::bail!("sysdll-cli binary not found")
    }

    fn backup_root() -> anyhow::Result<PathBuf> {
        let local = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("no LOCALAPPDATA / USERPROFILE"))?;
        let root = PathBuf::from(local).join("SysDll").join("backup");
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    struct ChildHandle {
        child: Child,
        #[allow(dead_code)]
        stdin: Mutex<ChildStdin>,
    }

    #[derive(Default)]
    pub struct ChildState {
        child: Mutex<Option<ChildHandle>>,
    }
}
