import { beforeEach, describe, expect, it, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useScanStore } from '../scan'

const fakeScanReport = {
  targets: [],
  files: [],
  total_files: 0,
  parsed_files: 0,
  failed_files: 0,
  duration_ms: 0,
}

const fakeDiagnostics = [
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
] as const

beforeEach(() => {
  setActivePinia(createPinia())
})

describe('useScanStore', () => {
  it('starts empty', () => {
    const store = useScanStore()
    expect(store.report).toBeNull()
    expect(store.diagnostics).toEqual([])
    expect(store.issueCount).toBe(0)
  })

  it('aggregates severity counts from diagnostics', () => {
    const store = useScanStore()
    store.diagnostics = [...fakeDiagnostics] as any
    expect(store.severityCount).toEqual({
      critical: 1,
      error: 0,
      warning: 0,
      info: 1,
    })
  })

  it('records scan error when invoke rejects', async () => {
    const store = useScanStore()
    vi.mock('@tauri-apps/api/core', () => ({
      invoke: vi.fn().mockRejectedValueOnce(new Error('boom')),
    }))

    // Re-import the store after mocking to pick up the mocked module.
    vi.resetModules()
    const { useScanStore: useScanStoreMocked } = await import('../scan')
    setActivePinia(createPinia())
    const s = useScanStoreMocked()

    await s.runScan([{ path: '/nope', recursive: true }])
    expect(s.scanError).toBe('Error: boom')
    expect(s.report).toBeNull()
  })

  it('populates report and diagnostics on success', async () => {
    vi.doMock('@tauri-apps/api/core', () => ({
      invoke: vi.fn()
        .mockResolvedValueOnce(fakeScanReport)
        .mockResolvedValueOnce(fakeDiagnostics),
    }))

    vi.resetModules()
    const { useScanStore: useScanStoreMocked } = await import('../scan')
    setActivePinia(createPinia())
    const s = useScanStoreMocked()

    await s.runScan([{ path: 'C:\\Windows\\System32', recursive: true }])
    expect(s.report).toEqual(fakeScanReport)
    expect(s.diagnostics.length).toBe(2)
    expect(s.issueCount).toBe(2)
  })
})
