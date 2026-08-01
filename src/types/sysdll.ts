// Mirrors of Rust types in sysdll-core. Keep field names identical so JSON
// deserialization works out of the box (serde uses the same names by default).

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

export interface CliEvent {
  event: string
  [key: string]: unknown
}
