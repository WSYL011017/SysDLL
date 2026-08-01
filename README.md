# SysDLL

一个面向 Windows 的开源系统 DLL 扫描与修复工具。

作为商业 DLL 修复套件（如 DLL-files.com、DLL Suite、DirectX Repair 等）的免费替代品。SysDLL
扫描系统目录，构建 DLL 与 EXE 的依赖图，诊断缺失 / 损坏 / 孤立 / 循环依赖等异常，并通过
**独立的提权子进程**完成写入与注册表修复，所有变更自动备份、可一键回滚。

## 相比收费工具的核心差异

| 维度 | 说明 |
|------|------|
| 🧠 **真正的依赖图分析** | 基于 `pelite` 解析 PE 导入表，递归构建 DLL 依赖图，识别**孤儿 DLL**（系统不再引用但仍在）和**循环依赖**，收费工具只看单层 Import 缺失 |
| 🔐 **数字签名验证** | 可识别被恶意替换的 DLL（签名异常 / 哈希黑名单），避免安装来路不明的文件 |
| 🛡️ **KnownDLLs 注册表劫持检测** | 解析 `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\KnownDLLs`，识别 DLL 搜索顺序被劫持的痕迹 |
| ♻️ **可回滚的修复** | 修复前自动备份到 `%LOCALAPPDATA%\SysDll\backup\<时间戳>\`，支持一键恢复到任意历史版本 |
| 🖥️ **CLI 子命令模式** | 提供独立的 `sysdll-cli.exe` 供脚本批处理，例如 `sysdll scan --json` |
| 📦 **完全开源可审计** | 修复源数据可追溯到 Microsoft / 厂商官方包，拒绝黑盒 |

## 技术栈

| 层 | 选型 | 理由 |
|----|------|------|
| 前端 | Vue 3 + Composition API + UnoCSS + Pinia + VueUse | 现代化、响应式、暗色自适应，与 antfu 风格一致 |
| 后端 | Rust + Tauri 2 | 体积小、启动快、内存安全；比 Electron 包小一个数量级 |
| PE 解析 | [`pelite`](https://crates.io/crates/pelite) | 业界最成熟的 Rust PE 解析库，覆盖 PE32 / PE32+ / .NET |
| 并发 | `rayon` + `crossbeam-channel` | 扫描阶段并行分析，事件流式回传 |
| IPC | JSON-RPC over stdin/stdout | 主进程与提权子进程间的轻量级通信 |

## 架构

```
┌─────────────────────────────────────────┐
│   GUI (sysdll.exe)                      │  asInvoker，普通用户权限即可启动
│   - 扫描触发 / 目标选择 / 结果展示       │
│   - 修复策略选择 / 备份恢复             │
│   - 实时日志流                           │
└──────────────────┬──────────────────────┘
                   │ Tauri invoke
                   ▼
┌─────────────────────────────────────────┐
│   IPC 路由层 (src-tauri/src/lib.rs)     │
└──────────────────┬──────────────────────┘
                   │ JSON-RPC / stdin-stdout
                   ▼
┌─────────────────────────────────────────┐
│   sysdll-cli.exe (提权子进程)           │  requireAdministrator 清单
│   - 写 System32 / SysWOW64              │  ShellExecuteW("runas") 启动
│   - 修改注册表 KnownDLLs                │
│   - 创建 / 恢复备份                     │
└─────────────────────────────────────────┘
```

### 为什么 GUI 不直接提权？

Tauri 2 在 Windows 11 24H2+ 的 Administrator Protection 下，如果 WebView2 进程以管理员权限启动会直接拒绝运行（见
[tauri-apps/tauri#13926](https://github.com/tauri-apps/tauri/issues/13926)）。因此本项目采用
**双进程架构**：GUI 始终以普通用户身份运行，所有需要写系统目录的操作委托给独立的 `sysdll-cli.exe`
子进程提权完成。

## 仓库结构

```
src-tauri/
├── Cargo.toml                    workspace 根
├── src/                          Tauri GUI 主进程 (crate `sysdll`)
│   ├── lib.rs                    Tauri IPC 命令入口
│   └── main.rs
├── crates/
│   ├── sysdll-core/              共享引擎 (无 UI 依赖)
│   │   ├── src/pe.rs             pelite PE 解析封装
│   │   ├── src/scan.rs           目录扫描 + 并行 PE 分析
│   │   ├── src/graph.rs          DLL 依赖图 (节点 + 边 + 环检测)
│   │   └── src/diag.rs           诊断规则 (缺失 / 孤立 / 循环)
│   └── sysdll-cli/               提权子进程
│       ├── build.rs              注入 requireAdministrator 清单
│       ├── resources/manifest.xml
│       └── src/{main,ipc,backup,fix}.rs
└── capabilities/                 Tauri 权限声明
```

## 开发

环境要求：Node.js ≥ 18、Rust ≥ 1.75、Tauri CLI。

```bash
# 安装前端依赖
npm install

# 启动 GUI 开发模式 (会自动启动 Vite + cargo build)
npm run tauri dev

# 单独运行测试
npm test                                # vitest 前端单元测试
cd src-tauri && cargo test -p sysdll-core   # Rust 核心引擎测试
```

## 命令行 (高级用户)

```bash
# 扫描指定目录并以 JSON 输出
sysdll-cli.exe scan --path C:\Windows\System32 --format json

# 查看备份列表
sysdll-cli.exe backup list

# 从指定备份恢复一个 DLL
sysdll-cli.exe backup restore --id 2026-08-02T03-12-45 --target user32.dll
```

## 路线图

- [x] MVP: 扫描 + 依赖图 + 基础诊断
- [ ] 已知 DLL 哈希库 (内置 Microsoft 签名版本基线)
- [ ] 数字签名验证 (Authenticode)
- [ ] KnownDLLs 注册表劫持检测
- [ ] 云端 DLL 源下载 (Microsoft Update Catalog / 厂商源)
- [ ] 修复计划可视化 (依赖图渲染)
- [ ] 国际化 (i18n)

## 贡献

欢迎 PR 与 Issue。所有改动请先跑 `npm test` 与 `cargo test -p sysdll-core` 确保通过。

## 许可证

Apache License 2.0 — 详见 [LICENSE](LICENSE)。

---

📖 [English version](README.en.md)
