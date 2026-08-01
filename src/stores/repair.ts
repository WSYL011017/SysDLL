import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { CliEvent } from '~/types/sysdll'

export interface LogLine {
  level: 'info' | 'warn' | 'error' | 'debug'
  message: string
  at: number
}

export const useRepairStore = defineStore('repair', () => {
  const cliRunning = ref(false)
  const logs = ref<LogLine[]>([])
  const lastFixResult = ref<{ target: string; backup: string } | null>(null)

  function appendLog(evt: CliEvent) {
    const now = Date.now()
    if (evt.event === 'log') {
      logs.value.push({
        level: (evt.level as LogLine['level']) ?? 'info',
        message: String(evt.message ?? ''),
        at: now,
      })
    }
    else if (evt.event === 'error') {
      logs.value.push({ level: 'error', message: String(evt.message ?? ''), at: now })
    }
    else if (evt.event === 'fix_done') {
      lastFixResult.value = {
        target: String(evt.target ?? ''),
        backup: String(evt.backup ?? ''),
      }
      logs.value.push({
        level: 'info',
        message: `fixed ${evt.target} (backup: ${evt.backup})`,
        at: now,
      })
    }
    else if (evt.event === 'scan_done') {
      logs.value.push({ level: 'info', message: 'scan completed', at: now })
    }

    // keep the buffer bounded
    if (logs.value.length > 500) {
      logs.value = logs.value.slice(-500)
    }
  }

  function clearLogs() {
    logs.value = []
    lastFixResult.value = null
  }

  return {
    cliRunning,
    logs,
    lastFixResult,
    appendLog,
    clearLogs,
  }
})
