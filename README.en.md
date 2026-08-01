# SysDLL

System DLL scanner & repair tool for Windows.

A free, open-source alternative to commercial DLL repair suites. Scans system
directories, builds a dependency graph of every DLL and EXE, diagnoses missing
/ broken / orphan references, and repairs through an elevated helper process
with automatic backup and rollback.

## Why SysDLL?

| Capability | Notes |
|------------|-------|
| Real dependency graph | `pelite`-based PE parsing builds a graph of all imports/exports; detects orphan DLLs and circular dependencies — paid tools usually only look at the first Import level. |
| Digital signature verification | Identifies DLLs swapped by malware (broken signatures / hash blocklists). |
| KnownDLLs hijack detection | Parses `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\KnownDLLs` to surface search-order tampering. |
| Rollable repairs | Every write is backed up under `%LOCALAPPDATA%\SysDll\backup\<timestamp>\` and can be restored with one command. |
| CLI subcommand mode | Standalone `sysdll-cli.exe` for scripts and batch jobs, e.g. `sysdll scan --json`. |
| Auditable, open source | Repair sources are traceable to Microsoft / vendor packages — no black boxes. |

## Stack

- **Frontend**: Vue 3 + Composition API + UnoCSS + Pinia + VueUse
- **Backend**: Rust + Tauri 2
- **PE parser**: [`pelite`](https://crates.io/crates/pelite)
- **GUI process**: runs `asInvoker` (no UAC prompt)
- **Repair process**: separate elevated child (`sysdll-cli.exe`) launched via
  `ShellExecuteW("runas")`

## Architecture

```
+-----------------------------------+
|   GUI (sysdll.exe)                |  asInvoker, Vue 3 + UnoCSS
|   - 扫描触发 / 结果展示             |
|   - 修复策略选择                   |
+---------------------+-------------+
                      | Tauri invoke
                      v
+-----------------------------------+
|   IPC Router (src-tauri/lib.rs)   |
+---------------------+-------------+
                      | JSON-RPC / stdin-stdout
                      v
+-----------------------------------+
|   sysdll-cli.exe                  |  requireAdministrator
|   - 写 System32 / SysWOW64        |
|   - 修改注册表 KnownDLLs          |
|   - 创建 / 恢复备份               |
+-----------------------------------+
```

Tauri 2 + WebView2 conflicts with Windows 11 24H2 Administrator Protection
when launched elevated, see
[tauri-apps/tauri#13926](https://github.com/tauri-apps/tauri/issues/13926).
The dual-process architecture keeps the GUI user-level and delegates all
system writes to a separate elevated child.

## Workspace layout

```
src-tauri/
├── Cargo.toml                workspace root
├── src/                      Tauri GUI host (crate `sysdll`)
└── crates/
    ├── sysdll-core/          shared engine (PE parsing, scan, graph, diag)
    └── sysdll-cli/           elevated child (requireAdministrator)
```

## Development

Requires Node.js ≥ 18 and Rust ≥ 1.75.

```bash
npm install
npm run tauri dev                       # GUI dev server
npm test                                # vitest (frontend)
cd src-tauri && cargo test -p sysdll-core   # core unit tests
```

## CLI (advanced users)

```bash
sysdll-cli.exe scan --path C:\Windows\System32 --format json
sysdll-cli.exe backup list
sysdll-cli.exe backup restore --id 2026-08-02T03-12-45 --target user32.dll
```

## Roadmap

- [x] MVP: scan + dependency graph + basic diagnostics
- [ ] Known DLL hash database
- [ ] Authenticode signature verification
- [ ] KnownDLLs hijack detection
- [ ] Cloud source download (Microsoft Update Catalog / vendor sources)
- [ ] Repair plan visualization (graph rendering)
- [ ] Internationalization (i18n)

## Contributing

PRs and Issues welcome. Please run `npm test` and `cargo test -p sysdll-core`
before opening a PR.

## License

Apache License 2.0 — see [LICENSE](LICENSE).

---

📖 [中文版](README.md)
