import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Diagnostic, ScanReport, ScanProgress, ScanTarget, Severity } from '~/types/sysdll'
import { useTauriCommand } from '~/composables/useTauriCommand'

const EMPTY_SEVERITY_COUNT: Record<Severity, number> = {
  critical: 0,
  error: 0,
  warning: 0,
  info: 0,
}

export const useScanStore = defineStore('scan', () => {
  // -- internal state --------------------------------------------------------
  // `diagnostics` is intentionally a ref (not exposed for write) so the
  // store can keep the only mutation paths behind `runScan` / `reset`. Audit
  // fixes P1-5: callers no longer poke `store.diagnostics = ...` directly.

  const _report = ref<ScanReport | null>(null)
  const _diagnostics = ref<Diagnostic[]>([])
  const scanError = ref<string | null>(null)
  const scanning = ref(false)
  const progress = ref<ScanProgress>({ scanned: 0, total: 0 })

  const scanCmd = useTauriCommand('scan_targets')
  const diagCmd = useTauriCommand('run_diagnostics')

  // -- external reads --------------------------------------------------------
  const report = computed(() => _report.value)
  const diagnostics = computed(() => _diagnostics.value)
  const issueCount = computed(() => _diagnostics.value.length)

  const severityCount = computed<Record<Severity, number>>(() => {
    const out: Record<Severity, number> = { ...EMPTY_SEVERITY_COUNT }
    for (const d of _diagnostics.value) {
      // Audit fix P2-13: exhaustiveness is enforced by `Record<Severity, ...>`,
      // so a brand-new severity value would be a TS error rather than NaN.
      out[d.severity]++
    }
    return out
  })

  function setProgress(next: ScanProgress): void {
    progress.value = next
  }

  // -- mutations -------------------------------------------------------------
  function reset(): void {
    _report.value = null
    _diagnostics.value = []
    scanError.value = null
    progress.value = { scanned: 0, total: 0 }
  }

  async function runScan(targets: ScanTarget[]): Promise<void> {
    scanning.value = true
    scanError.value = null
    progress.value = { scanned: 0, total: 0 }
    _diagnostics.value = []

    try {
      const reportRes = await scanCmd.run({ targets })
      if (!reportRes) {
        scanError.value = scanCmd.error.value ?? 'unknown error'
        return
      }
      _report.value = reportRes

      const diags = await diagCmd.run({})
      // Audit fix P2-8: diagnostics are cleared even on failure so the UI
      // doesn't show a stale list labelled "Issues · N".
      _diagnostics.value = diags ?? []
      if (!diags) {
        scanError.value = diagCmd.error.value ?? scanError.value ?? 'diagnostics failed'
      }
    }
    finally {
      scanning.value = false
    }
  }

  return {
    // external reads
    report,
    diagnostics,
    issueCount,
    severityCount,
    progress,
    // status
    scanning,
    scanError,
    // mutations
    runScan,
    setProgress,
    reset,
  }
})
