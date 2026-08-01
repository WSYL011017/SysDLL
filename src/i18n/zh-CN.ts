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

export const t = {
  // App.vue
  app: {
    statCritical: '严重',
    statError: '错误',
    statWarn: '警告',
    statInfo: '信息',
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
  },

  // RepairLog.vue
  repairLog: {
    emptyHint: '暂无活动日志。',
  },

  // Severity enum (data contract) -> display label
  severity: {
    critical: '严重',
    error: '错误',
    warning: '警告',
    info: '信息',
  },

  // DiagnosticKind enum (data contract) -> short Chinese label
  kind: {
    missing_dll: '缺失 DLL',
    orphan_dll: '孤儿 DLL',
    broken_pe_parse: 'PE 解析失败',
    circular_dependency: '循环依赖',
    parse_failure: '解析错误',
  },

  // CliEvent.level enum -> display label
  logLevel: {
    info: '信息',
    warn: '警告',
    error: '错误',
  },
} as const

export type Dict = typeof t