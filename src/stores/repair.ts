import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { CliEvent, LogLevel, ScanProgress } from '~/types/sysdll'

export interface LogLine {
  level: LogLevel
  message: string
  at: number
}

const MAX_LINES = 500

export const useRepairStore = defineStore('repair', () => {
  const cliRunning = ref(false)
  const logs = ref<LogLine[]>([])
  const lastFixResult = ref<{ target: string; backup: string } | null>(null)
  const scanProgress = ref<ScanProgress>({ scanned: 0, total: 0 })

  function pushLine(level: LogLevel, message: string, at: number): void {
    logs.value.push({ level, message, at })
    if (logs.value.length > MAX_LINES) {
      // keep the tail; allocate once so we don't recreate every push
      logs.value = logs.value.slice(-MAX_LINES)
    }
  }

  /**
   * Audit fix P2-10: switch on the discriminated union so adding a new
   * CliEvent variant is a TypeScript exhaustiveness error here, not a silent
   * no-op.
   */
  function appendLog(evt: CliEvent): void {
    const now = Date.now()
    switch (evt.event) {
      case 'log':
        pushLine(evt.level, evt.message, now)
        break
      case 'error':
        pushLine('error', evt.message, now)
        break
      case 'fix_done':
        lastFixResult.value = { target: evt.target, backup: evt.backup }
        pushLine('info', `fixed ${evt.target} (backup: ${evt.backup})`, now)
        break
      case 'restore_done':
        pushLine('info', `restored ${evt.target} (backup: ${evt.backup})`, now)
        break
      case 'scan_done':
        pushLine('info', `scan completed: ${evt.report.total_files} files`, now)
        break
      case 'progress':
        scanProgress.value = {
          scanned: evt.scanned,
          total: evt.total,
          current: evt.current,
        }
        break
      case 'pong':
        // The presence of a pong is itself the signal; the user sees the
        // lastFixResult/appendLog timing in the log. Quietly no-op.
        break
      case 'exit':
        pushLine('warn', `cli exited (code=${evt.code ?? 'killed'})`, now)
        cliRunning.value = false
        break
      default: {
        // Exhaustiveness guard. If a new CliEvent variant ships without
        // updating this switch, tsc fails the build.
        const _exhaustive: never = evt
        void _exhaustive
      }
    }
  }

  function clearLogs(): void {
    logs.value = []
    lastFixResult.value = null
    scanProgress.value = { scanned: 0, total: 0 }
  }

  return {
    cliRunning,
    logs,
    lastFixResult,
    scanProgress,
    appendLog,
    clearLogs,
  }
})
