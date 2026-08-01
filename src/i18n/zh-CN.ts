// Centralized UI strings for the zh-CN locale.
//
// Why a dictionary file instead of inline strings:
//   1. Single source of truth for translators / future i18n switches.
//   2. Vue templates stay terse and grep-friendly.
//   3. Mirrors the backend log messages so the GUI never re-implements
//      the same wording.
//
// What stays English by design:
//   - data contract field names (`severity: 'critical'`, `kind: 'missing_dll'`)
//     are mirrored from Rust serde structs and must remain identical for
//     IPC JSON deserialization.
//   - file paths, SHA256 hashes, and other machine values are never
//     translated.
//
// Mapping tables (severity / kind / logLevel) are typed `Record<Enum, string>`
// so adding a Rust enum variant without a matching translation is a compile
// error instead of a silent missing label at runtime.

import type { DiagnosticKind, Severity, LogLevel } from '~/types/sysdll'

export const t = {
  // App.vue
  app: {
    // P1-13: collapse parallel labels into the severity dictionary so the
    // header counter, badge, and any future severity UI all read from one key.
    startRepair: '开始提权修复',
    issuesHeading: (n: number) => `问题列表 · ${n}`,
    selectIssueHint: '选择左侧问题查看详情',
    repairLogHeading: '修复日志',
    clearLog: '清空',
  },

  // ScanTargetPicker.vue
  scanPicker: {
    defaultTargets: '默认扫描目标',
    recursiveTag: '递归',
    extraPathsHeading: '额外路径 (每行一个)',
    extraPathsPlaceholder: 'C:\\Program Files\\MyApp',
    runScan: '开始扫描',
    scanning: '扫描中...',
    statScanned: '已扫描',
    statParsed: '已解析',
    statFailed: '失败',
    statDuration: '耗时',
    statMs: '毫秒',
  },

  // ScanProgress.vue
  scanProgress: {
    scanning: (scanned: number, total: number) =>
      `扫描中 ${scanned} / ${total}`,
    idleHint: '进度将以当前扫描事件流式刷新。',
  },

  // IssueList.vue
  issueList: {
    affects: (n: number) => `影响 ${n} 个模块`,
    emptyHint: '暂无问题。运行一次扫描以开始诊断。',
  },

  // IssueDetail.vue
  issueDetail: {
    subject: '目标对象',
    relatedFiles: '相关文件',
    dependents: '依赖此对象的上游模块',
    moreDependents: (n: number) => `还有 ${n} 个`,
    downloadMicrosoft: '从微软下载',
    restoreBackup: '从备份恢复',
    restoreConfirm: (target: string) => `确认从最近一次备份恢复 ${target}？`,
  },

  // RepairLog.vue
  repairLog: {
    emptyHint: '暂无活动日志。',
    unknownEvent: (payload: unknown) =>
      `未识别事件: ${JSON.stringify(payload)}`,
  },

  // Severity enum (data contract) -> display label
  severity: {
    critical: '严重',
    error: '错误',
    warning: '警告',
    info: '信息',
  } satisfies Record<Severity, string>,

  // DiagnosticKind enum (data contract) -> short Chinese label
  kind: {
    missing_dll: '缺失 DLL',
    orphan_dll: '孤儿 DLL',
    broken_pe_parse: 'PE 解析失败',
    circular_dependency: '循环依赖',
    parse_failure: '解析错误',
  } satisfies Record<DiagnosticKind, string>,

  // CliEvent.level enum -> display label
  logLevel: {
    info: '信息',
    warn: '警告',
    error: '错误',
  } satisfies Record<LogLevel, string>,
} as const

export type Dict = typeof t
