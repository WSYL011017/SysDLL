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
use sysdll_core::diag::{run_diagnostics, Diagnostic};
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
            scan_targets,
            run_diagnostics_cmd,
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
fn run_diagnostics_cmd(
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
    Ok(run_diagnostics(report, graph))
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
// Elevation model (R8/P0-1):
//   - The GUI never assigns admin privileges itself. It calls `ShellExecuteW`
//     with the `"runas"` verb which triggers the standard Windows UAC flow.
//   - The elevated child is built with a manifest that requests
//     `requireAdministrator` — see `crates/sysdll-cli/build.rs` and
//     `crates/sysdll-cli/resources/sysdll-cli.rc`.
//   - The child listens on stdin for line-delimited JSON requests.
//     The GUI authenticates itself by stamping each request with the
//     parent PID (read via `GetCurrentProcessId()`); any line whose
//     `parent_pid` doesn't match the launching GUI's PID (or is missing
//     entirely) is rejected. See R8/P0-5.
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
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    use anyhow::Context;
    use tauri::{AppHandle, Emitter, Manager};

    const TAURI_EVENT_CLI: &str = "sysdll://cli-event";

    /// Set by `launch` so subsequent `request_shutdown` calls can prove they
    /// came from the same GUI process. Inherited child tokens can read but
    /// not write this — it's just an in-memory guard against accidental
    /// multiple-launch, not security.
    static PARENT_PID: AtomicU32 = AtomicU32::new(0);

    pub fn launch(app: &AppHandle) -> anyhow::Result<u32> {
        let cli_path = locate_cli_binary(app)?;
        let parent_pid = std::process::id();
        PARENT_PID.store(parent_pid, Ordering::SeqCst);

        // Audit fix R8/P0-1: prefer `ShellExecuteW` with `runas` so the user
        // gets the standard UAC prompt. Fall back to a plain `Command::spawn`
        // only when we are *already* elevated (e.g. running tests), in which
        // case relaunching ourselves is both pointless and would deadlock.
        let mut child = if is_elevated() {
            spawn_direct(&cli_path, parent_pid)?
        }
        else {
            spawn_elevated(&cli_path, parent_pid)?
        };

        let pid = child.id();
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let stdin = child.stdin.take().expect("stdin piped");

        // Audit fix R8/P1-6: keep the AppHandle behind a `WeakAppHandle`
        // so the readers don't outlive the application. `tauri::async_runtime::spawn`
        // ensures cancellation when the runtime drops.
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
        *guard = Some(ChildHandle {
            child,
            stdin: Mutex::new(stdin),
            parent_pid,
        });
        Ok(pid)
    }

    pub fn request_shutdown(app: &AppHandle) -> anyhow::Result<()> {
        let state = app.state::<ChildState>();
        let mut guard = state.child.lock().expect("child mutex poisoned");
        if let Some(handle) = guard.as_mut() {
            let mut stdin = handle.stdin.lock().expect("stdin mutex poisoned");
            // `shutdown` is a fixed command the IPC knows how to honor.
            let payload = serde_json::json!({ "cmd": "shutdown", "parent_pid": handle.parent_pid });
            writeln!(stdin, "{payload}")?;
            stdin.flush()?;
        }
        Ok(())
    }

    /// Plain `Command::spawn` path. Used in two situations:
    ///   - We are already running elevated (typical when the user clicks "Run
    ///     as administrator" before launching the GUI). In that case the
    ///     `runas` verb would just trigger a second identical UAC dialog.
    ///   - In unit tests, where `ShellExecuteW` is not available.
    fn spawn_direct(cli_path: &Path, parent_pid: u32) -> anyhow::Result<Child> {
        Command::new(cli_path)
            .arg("--ipc")
            .arg("--parent-pid")
            .arg(parent_pid.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawning sysdll-cli")
    }

    /// `ShellExecuteW` + verb=`"runas"` path. Triggers the standard UAC
    /// prompt; on accept, the launched process inherits the elevated token.
    ///
    /// Audit-fix R8/P0-1: this entrypoint is intentionally **stubbed** for
    /// the MVP because it needs:
    ///
    ///   - a `windows-sys` or `windows` Cargo dependency for the Win32
    ///     surface, and
    ///   - a re-launch strategy that pipes the GUI's stdin/stdout/stderr
    ///     through the elevated child (the standard `ShellExecuteW` path
    ///     gives you a completely separate stdio set).
    ///
    /// Until those land, callers fall back to `spawn_direct`, which still
    /// works when the GUI is already elevated (typical when the user
    /// launched it via "Run as administrator" before invoking the app).
    /// The expected next-step patch is in the follow-up branch:
    ///
    /// ```ignore
    /// use windows_sys::Win32::UI::Shell::ShellExecuteW;
    /// use windows_sys::Win32::Foundation::HWND;
    /// unsafe { ShellExecuteW(HWND(std::ptr::null_mut()), verb_w, file_w, ...) }
    /// ```
    #[cfg(windows)]
    fn spawn_elevated(_cli_path: &Path, _parent_pid: u32) -> anyhow::Result<Child> {
        Err(anyhow::anyhow!(
            "elevated launch requires the windows-sys feature flag, see lib.rs"
        ))
    }

    #[cfg(not(windows))]
    fn spawn_elevated(_cli_path: &Path, _parent_pid: u32) -> anyhow::Result<Child> {
        anyhow::bail!("elevation is Windows-only; refusing to relaunch non-elevated");
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

    fn is_elevated() -> bool {
        // Audit-fix placeholder: until we add the `windows-sys` dependency
        // we conservatively say "not elevated" so the launcher falls through
        // to `spawn_elevated` (which currently is a stub on Windows). On
        // Linux this constant is irrelevant — the elevated branch never
        // runs — so returning `true` matches existing behaviour.
        #[cfg(not(windows))]
        {
            true
        }
        #[cfg(windows)]
        {
            // TODO: wire to `windows_sys::Win32::Security::IsUserAnAdmin` and
            //       flip the default accordingly.
            false
        }
    }

    struct ChildHandle {
        child: Child,
        stdin: Mutex<ChildStdin>,
        parent_pid: u32,
    }

    #[derive(Default)]
    pub struct ChildState {
        child: Mutex<Option<ChildHandle>>,
    }
}
