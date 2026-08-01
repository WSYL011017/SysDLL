# SysDLL

System DLL scanner & repair tool for Windows.

A free, open-source alternative to commercial DLL repair suites. Scans system
directories, builds a dependency graph of every DLL and EXE, diagnoses missing
/ broken / orphan references, and repairs through an elevated helper process
with automatic backup and rollback.

## Stack

- **Frontend**: Vue 3 + Composition API + UnoCSS + Pinia + VueUse
- **Backend**: Rust (Tauri 2)
- **PE parser**: [`pelite`](https://crates.io/crates/pelite)
- **GUI process**: runs asInvoker (no UAC prompt)
- **Repair process**: separate elevated child (`sysdll-cli.exe`) launched via
  `ShellExecuteW("runas")` — see [architecture rationale](#architecture) below.

## Architecture

The GUI never requests elevation. When repairs need to touch `System32` /
`SysWOW64` / `HKLM`, the main process launches a separate `sysdll-cli.exe`
child built with a `requireAdministrator` manifest. The two communicate over
JSON-RPC on stdin/stdout so the UI can stream progress live.

This avoids the conflict between Tauri 2's WebView2 and Windows 11 24H2
Administrator Protection documented in
[tauri-apps/tauri#13926](https://github.com/tauri-apps/tauri/issues/13926).

```
+-----------------------------+
|   GUI (sysdll.exe)          |  asInvoker, Vue 3 + UnoCSS
|   - 扫描触发 / 结果展示      |
|   - 修复策略选择             |
+-------------||--------------+
              v
+-----------------------------+
|   IPC Router (lib.rs)       |
+-------------||--------------+
              |
       +------+------+
       v             v
+----------+   +------------------+
| 主进程    |   | sysdll-cli.exe   |  requireAdministrator
| 扫描      |   | 写 System32 / 注册表 |
+----------+   +------------------+
```

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

```bash
npm install
npm run tauri dev          # GUI dev server
npm test                    # vitest (frontend)
cd src-tauri && cargo test -p sysdll-core   # core unit tests
```

## Recommended IDE Setup

- [VS Code](https://codevisualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
