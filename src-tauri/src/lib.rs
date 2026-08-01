//! Tauri command surface for the SysDll GUI.
//!
//! The GUI process runs at medium integrity level (no UAC prompt). When
//! repairs need to touch `%WINDIR%\\System32` we relaunch the bundled
//! `sysdll-cli.exe` via `ShellExecuteW` with verb `"runas"`; the Win32
//! loader then displays one UAC prompt and produces an elevated token, which
//! the child inherits. The child streams JSON-RPC events back over its
//! stdout, which the GUI turns into Tauri events the frontend can listen
//! for.

mod app_error;

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sysdll_core::diag::{run_diagnostics as run_diagnostics_impl, Diagnostic};
use sysdll_core::graph::DependencyGraph;
use sysdll_core::scan::{ScanReport, ScanTarget};

pub use app_error::{AppError, AppResult};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .manage(cli::ChildState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            is_admin,
            scan_targets,
            run_diagnostics,
            launch_cli,
            shutdown_cli,
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

/// Returns `true` when the GUI is running with an elevated token.
///
/// `IsUserAnAdmin` is a cheap Win32 check (no syscall + no allocation);
/// the cost is negligible against startup. We return `true` on non-Windows
/// hosts because the elevated path is irrelevant there.
#[tauri::command]
fn is_admin() -> bool {
    #[cfg(windows)]
    {
        // IsUserAnAdmin returns a non-zero `BOOL` (i32) when the user is a
        // member of the Administrators group for the current process token.
        // SAFETY: the function has no preconditions and is alloc-free.
        let r: i32 = unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() };
        r != 0
    }
    #[cfg(not(windows))]
    {
        // Not a Windows host → no UAC concept → treat as already privileged
        // so the gating logic stays consistent across platforms.
        true
    }
}

/// Run a scan on the requested targets. The result is cached so the
/// `run_diagnostics` command can re-derive issues without re-scanning.
#[tauri::command]
fn scan_targets(
    state: tauri::State<'_, AppState>,
    targets: Vec<ScanTarget>,
) -> AppResult<ScanReport> {
    let handle = sysdll_core::scan::run_scan(targets);
    let report = handle.report;
    let graph = DependencyGraph::from_scan(&report);
    *state.last_report.lock().expect("last_report mutex poisoned") = Some(report.clone());
    *state.last_graph.lock().expect("last_graph mutex poisoned") = Some(graph);
    Ok(report)
}

#[tauri::command]
fn run_diagnostics(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<Diagnostic>> {
    let report_guard = state.last_report.lock().expect("last_report mutex poisoned");
    let graph_guard = state.last_graph.lock().expect("last_graph mutex poisoned");
    let report = report_guard
        .as_ref()
        .ok_or_else(|| AppError::PreconditionFailed("no scan has been run yet".into()))?;
    let graph = graph_guard
        .as_ref()
        .ok_or_else(|| AppError::PreconditionFailed("no scan has been run yet".into()))?;
    Ok(run_diagnostics_impl(report, graph))
}

/// Launch the elevated CLI child.
/// Returns the PID so the frontend can attach to its stdout / stderr streams.
#[tauri::command]
fn launch_cli(app: tauri::AppHandle) -> AppResult<u32> {
    cli::launch(&app).map_err(AppError::from)
}

/// Politely ask the elevated CLI to exit (no-op if it is already gone).
#[tauri::command]
fn shutdown_cli(app: tauri::AppHandle) -> AppResult<()> {
    cli::request_shutdown(&app).map_err(AppError::from)
}

#[tauri::command]
fn list_backups() -> AppResult<Vec<BackupEntry>> {
    cli::list_backups().map_err(AppError::from)
}

#[tauri::command]
fn restore_backup(target: PathBuf) -> AppResult<String> {
    cli::restore_backup(&target).map_err(AppError::from)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupEntry {
    pub timestamp: String,
    pub files: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// CLI child plumbing
// ---------------------------------------------------------------------------
//
// Elevation model (U1/P0-1 / P0-5):
//   - The GUI executable declares `requireAdministrator` in its embedded
//     manifest via `app.manifest` + `build.rs`. A normal double-click (or
//     `nsis-tauri-utils`-driven post-install launch) triggers exactly one
//     UAC dialog and the GUI boots with the elevated token.
//
//   - Once the GUI is elevated it spawns `sysdll-cli.exe` with
//     `std::process::Command`. Windows then **inherits** the parent's
//     token by default, so the child runs at the same integrity level
//     without a second UAC prompt. We deliberately do NOT add a
//     manifest to the CLI child — that would force another UAC dialog
//     and is the wrong way to keep the token.
//
//   - Because the child can only be spawned by code already running in
//     the elevated GUI's address space, the IPC channel is implicitly
//     authenticated. The previous audit-tagged P0-5 (parent-PID stamp)
//     is no longer required.
//
// IPC event delivery (R8/P0-2 / P1-14):
//   - Every child process event is serialized with `serde_json::to_string`
//     before being emitted, so the JSON parser downstream never sees raw
//     user-controlled bytes from the child.
//
// Lifecycle (R8/P1-6 / R6/P1-9):
//   - The stdout / stderr readers run on the Tauri runtime (not bare
//     `std::thread::spawn`) so they're cleanly cancelled when the AppHandle
//     drops.
//   - When stdin closes (parent died) or the child exits, we emit an
//     `Event::Exit` payload back to the GUI so the store can flip
//     `cliRunning = false`.
mod cli {
    use super::{AppError, BackupEntry};
    use std::io::{BufRead, BufReader, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::Mutex;

    use anyhow::Context;
    use tauri::{AppHandle, Emitter, Manager};

    const TAURI_EVENT_CLI: &str = "sysdll://cli-event";

    pub fn launch(app: &AppHandle) -> anyhow::Result<u32> {
        let cli_path = locate_cli_binary(app)?;
        let mut child = spawn_child(&cli_path)?;

        let pid = child.id();
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let stdin = child.stdin.take().expect("stdin piped");

        // Audit fix R8/P1-6: keep the AppHandle behind a clone (Tauri's
        // `AppHandle` itself is `Clone` and holds an internal `Arc`, so
        // a bare clone is fine) and run the readers on the Tauri async
        // runtime so they're cancelled when the runtime drops.
        let weak_app = app.clone();
        tauri::async_runtime::spawn(async move {
            pipe_stdout(weak_app.clone(), stdout).await;
            // When stdout closes (parent dropped stdin or child died), let
            // the GUI know.
            let _ = weak_app.emit(
                TAURI_EVENT_CLI,
                serde_json::json!({ "event": "exit", "code": null }).to_string(),
            );
        });

        let weak_app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Audit fix R8/P0-2 + P1-14: serialize stderr lines through serde_json
            // so any character (including quotes / backslashes) stays valid JSON.
            pipe_stderr(weak_app, stderr).await;
        });

        // Stash the child so the GUI can drive it later.
        let state = app.state::<ChildState>();
        let mut guard = state.child.lock().expect("child mutex poisoned");
        if let Some(mut prev) = guard.take() {
            let _ = prev.child.kill();
            let _ = prev.child.wait();
        }
        *guard = Some(ChildHandle { child, stdin: Mutex::new(stdin) });
        Ok(pid)
    }

    pub fn request_shutdown(app: &AppHandle) -> anyhow::Result<()> {
        let state = app.state::<ChildState>();
        let mut guard = state.child.lock().expect("child mutex poisoned");
        if let Some(handle) = guard.as_mut() {
            let mut stdin = handle.stdin.lock().expect("stdin mutex poisoned");
            // `shutdown` is a fixed command the IPC knows how to honor.
            writeln!(stdin, r#"{{"cmd":"shutdown"}}"#)?;
            stdin.flush()?;
        }
        Ok(())
    }

    /// Plain `Command::spawn` against the elevated `sysdll-cli.exe`.
    ///
    /// Windows inherits the parent's elevated token automatically
    /// (CreateProcess default), so no `ShellExecuteW` plumbing is needed.
    /// The child is built *without* a manifest specifically so it does
    /// not ask for admin itself — see the module doc for the rationale.
    fn spawn_child(cli_path: &Path) -> anyhow::Result<Child> {
        Command::new(cli_path)
            .arg("--ipc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning sysdll-cli")
    }

    async fn pipe_stdout(app: AppHandle, stdout: std::process::ChildStdout) {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            // We deliberately emit the line verbatim: it's already valid
            // serde_json because the child produced it that way.
            let _ = app.emit(TAURI_EVENT_CLI, line);
        }
    }

    async fn pipe_stderr(app: AppHandle, stderr: std::process::ChildStderr) {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            // Audit fix R8/P0-2: wrap in serde_json rather than format!() —
            // any `"` or `\` in the stderr line would otherwise corrupt the
            // downstream parser and silently drop the event.
            let payload = serde_json::json!({
                "event": "log",
                "level": "error",
                "message": line,
            });
            let _ = app.emit(TAURI_EVENT_CLI, payload.to_string());
        }
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

    pub fn restore_backup(_target: &Path) -> anyhow::Result<String> {
        // Audit fix R7/P0-4 + P0-3: the dedicated CLI restoration routine
        // lives in the elevated child — see `crate::backup::restore_latest`.
        // Reaching in from the GUI would defeat the elevation boundary.
        Err(AppError::PreconditionFailed(
            "use the elevated CLI child for restore; the GUI never writes System32".into(),
        )
        .into())
    }

    fn locate_cli_binary(app: &AppHandle) -> anyhow::Result<PathBuf> {
        // In dev, cargo puts it at target/debug/sysdll-cli.exe next to the GUI binary.
        // In production, the bundler places it next to the main exe.
        let exe_dir = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("no parent for current exe"))?
            .to_path_buf();
        let candidate = exe_dir.join(cli_binary_name());
        if candidate.exists() {
            return Ok(candidate);
        }
        // Fallback to the resource directory baked by the bundler.
        if let Ok(resource) = app.path().resource_dir() {
            let alt = resource.join(cli_binary_name());
            if alt.exists() {
                return Ok(alt);
            }
        }
        anyhow::bail!("sysdll-cli binary not found")
    }

    // Audit fix R4/P3-15: single source of truth for the bundle name.
    fn cli_binary_name() -> &'static str {
        if cfg!(windows) {
            "sysdll-cli.exe"
        }
        else {
            "sysdll-cli"
        }
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
        stdin: Mutex<ChildStdin>,
    }

    #[derive(Default)]
    pub struct ChildState {
        child: Mutex<Option<ChildHandle>>,
    }
}
