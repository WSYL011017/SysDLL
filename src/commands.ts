// Single source of truth for every Tauri command this app calls.
//
// Why centralize it here:
//   1. Type-safety: a missing command or arg key is a compile error, not a
//      runtime "unknown command" (audit P1-2 / P1-3 / P2-4).
//   2. Args are passed as a single object literal that maps directly onto
//      the Rust tauri::command signature, instead of position tuples
//      smuggled through a stringly-typed KEY_MAP.
//   3. Mirrors `src-tauri/src/lib.rs` generate_handler! — drift shows up
//      during `cargo check` / `vue-tsc`.

import type { ScanReport, Diagnostic, ScanTarget, BackupEntry } from '~/types/sysdll'

/**
 * Map of Tauri command name -> argument object type.
 * Adding an entry below without updating both sides (lib.rs handler + here)
 * is a TypeScript compile error.
 */
export interface CommandMap {
  ping: Record<string, never>
  is_admin: Record<string, never>
  scan_targets: { targets: ScanTarget[] }
  run_diagnostics: Record<string, never>
  launch_cli: Record<string, never>
  shutdown_cli: Record<string, never>
  list_backups: Record<string, never>
  restore_backup: { target: string }
}

export type CommandName = keyof CommandMap
export type CommandArgs<C extends CommandName> = CommandMap[C]
export type CommandResult<C extends CommandName> =
  C extends 'ping' ? string :
  C extends 'is_admin' ? boolean :
  C extends 'scan_targets' ? ScanReport :
  C extends 'run_diagnostics' ? Diagnostic[] :
  C extends 'launch_cli' ? number :
  C extends 'shutdown_cli' ? void :
  C extends 'list_backups' ? BackupEntry[] :
  C extends 'restore_backup' ? string :
  never

/** Empty args for commands that take no payload. */
export const NO_ARGS = {} as const
