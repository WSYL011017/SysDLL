import { beforeEach, describe, expect, it, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'

// Mock before importing the store; the composable creates a binding to invoke()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { useScanStore } from '../scan'
import type { Diagnostic, ScanReport, ScanTarget } from '~/types/sysdll'

const fakeReport: ScanReport = {
  targets: [],
  files: [],
  total_files: 0,
  parsed_files: 0,
  failed_files: 0,
  duration_ms: 0,
}

const fakeDiagnostics: Diagnostic[] = [
  {
    kind: 'missing_dll',
    severity: 'critical',
    title: 'Missing kernel32',
    detail: '',
    subject: 'kernel32',
    related_paths: [],
    dependents: ['system'],
  },
  {
    kind: 'orphan_dll',
    severity: 'info',
    title: 'Orphan test.dll',
    detail: '',
    subject: 'test',
    related_paths: [],
    dependents: [],
  },
]

const targets: ScanTarget[] = [{ path: 'C:\\Windows\\System32', recursive: true }]

beforeEach(() => {
  setActivePinia(createPinia())
  vi.mocked(invoke).mockReset()
})

describe('useScanStore', () => {
  it('starts empty', () => {
    const store = useScanStore()
    expect(store.report).toBeNull()
    expect(store.diagnostics).toEqual([])
    expect(store.issueCount).toBe(0)
  })

  it('aggregates severity counts from diagnostics', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(fakeReport)
      .mockResolvedValueOnce(fakeDiagnostics)
    const store = useScanStore()

    await store.runScan(targets)
    expect(store.severityCount).toEqual({
      critical: 1,
      error: 0,
      warning: 0,
      info: 1,
    })
  })

  it('records scan error when scan_targets rejects', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('boom'))
    const store = useScanStore()

    await store.runScan(targets)
    expect(store.scanError).toBe('boom')
    expect(store.report).toBeNull()
    expect(store.diagnostics).toEqual([])
  })

  it('records diagnostics error when scan_targets succeeds but diagnostics rejects', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(fakeReport)
      .mockRejectedValueOnce(new Error('diag-boom'))
    const store = useScanStore()

    await store.runScan(targets)
    expect(store.report).toEqual(fakeReport)
    expect(store.scanError).toBe('diag-boom')
    expect(store.diagnostics).toEqual([])
  })

  it('populates report and diagnostics on success', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(fakeReport)
      .mockResolvedValueOnce(fakeDiagnostics)
    const store = useScanStore()

    await store.runScan(targets)
    expect(store.report).toEqual(fakeReport)
    expect(store.diagnostics).toHaveLength(2)
    expect(store.issueCount).toBe(2)
  })

  it('reset() clears everything', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(fakeReport)
      .mockResolvedValueOnce(fakeDiagnostics)
    const store = useScanStore()
    await store.runScan(targets)

    store.reset()
    expect(store.report).toBeNull()
    expect(store.diagnostics).toEqual([])
    expect(store.scanError).toBeNull()
  })

  it('setProgress() updates progress ref', () => {
    const store = useScanStore()
    store.setProgress({ scanned: 5, total: 10, current: 'x.dll' })
    expect(store.progress).toEqual({ scanned: 5, total: 10, current: 'x.dll' })
  })
})
