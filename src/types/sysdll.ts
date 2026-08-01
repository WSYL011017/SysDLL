// Mirrors of Rust types in sysdll-core. Keep field names identical so JSON
// deserialization works out of the box (serde uses the same names by default).
// Any enum variant added below must also be added to src/i18n/zh-CN.ts;
// the `satisfies Record<Enum, string>` constraint will refuse to compile
// otherwise.

export type Severity = 'info' | 'warning' | 'error' | 'critical'

export type PeKind = 'pe32' | 'pe32plus' | 'unknown'

export interface PeInfo {
  kind: PeKind
  machine: string
  imports: string[]
  exports: string[]
  sha256: string
  file_size: number
}

export interface ScannedFile {
  path: string
  size: number
  pe: PeInfo | null
  error: string | null
}

export interface ScanTarget {
  path: string
  recursive: boolean
}

// Granular scan progress emitted by the backend as a stream of events.
// The channel is only wired up if the caller asks for it; otherwise
// the report is the only payload.
export interface ScanProgress {
  scanned: number
  total: number
  current?: string
}

export interface ScanReport {
  targets: ScanTarget[]
  files: ScannedFile[]
  total_files: number
  parsed_files: number
  failed_files: number
  duration_ms: number
}

export type DiagnosticKind =
  | 'missing_dll'
  | 'orphan_dll'
  | 'broken_pe_parse'
  | 'circular_dependency'
  | 'parse_failure'

export interface Diagnostic {
  kind: DiagnosticKind
  severity: Severity
  title: string
  detail: string
  subject: string
  related_paths: string[]
  dependents: string[]
}

export interface BackupEntry {
  timestamp: string
  files: string[]
}

// IPC levels emitted by the elevated CLI child.
// Pin the set so exhaustiveness checks in appendLog stay honest.
export type LogLevel = 'info' | 'warn' | 'error'

// Discriminated union of every payload the GUI can receive from the IPC
// child. Each branch corresponds 1:1 to a `sysdll_core::ipc::Event`
// variant. Adding a Rust variant is a TypeScript compile error unless
// this union is updated in lockstep.
export type CliEvent =
  | { event: 'log'; level: LogLevel; message: string }
  | { event: 'scan_done'; report: ScanReport }
  | { event: 'fix_done'; target: string; backup: string }
  | { event: 'restore_done'; target: string; backup: string }
  | { event: 'progress'; scanned: number; total: number; current?: string }
  | { event: 'pong' }
  | { event: 'exit'; code: number | null }
  | { event: 'error'; message: string }

// Back-compat alias – the original loose type is kept around for any callers
// that still surface raw lines from the CLI before they deserialize.
// Prefer `CliEvent` going forward.
export interface LegacyCliEvent {
  event: string
  [key: string]: unknown
}
