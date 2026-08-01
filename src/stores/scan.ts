import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Diagnostic, ScanReport, ScanTarget } from '~/types/sysdll'
import { useTauriCommand } from '~/composables/useTauriCommand'

export const useScanStore = defineStore('scan', () => {
  const report = ref<ScanReport | null>(null)
  const diagnostics = ref<Diagnostic[]>([])
  const scanError = ref<string | null>(null)
  const scanning = ref(false)
  const progress = ref({ scanned: 0, total: 0 })

  const scanCmd = useTauriCommand<[ScanTarget[]], ScanReport>('scan_targets')
  const diagCmd = useTauriCommand<[], Diagnostic[]>('run_diagnostics_cmd')

  const issueCount = computed(() => diagnostics.value.length)

  const severityCount = computed(() => {
    const out = { critical: 0, error: 0, warning: 0, info: 0 }
    for (const d of diagnostics.value) {
      out[d.severity]++
    }
    return out
  })

  async function runScan(targets: ScanTarget[]): Promise<void> {
    scanning.value = true
    scanError.value = null
    progress.value = { scanned: 0, total: 0 }
    try {
      const result = await scanCmd.run(targets)
      if (!result) {
        scanError.value = scanCmd.error.value ?? 'unknown error'
        return
      }
      report.value = result
      const diags = await diagCmd.run()
      diagnostics.value = diags ?? []
    }
    finally {
      scanning.value = false
    }
  }

  return {
    report,
    diagnostics,
    scanError,
    scanning,
    progress,
    issueCount,
    severityCount,
    runScan,
  }
})
