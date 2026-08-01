import { beforeEach, describe, expect, it } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useRepairStore } from '../repair'
import type { CliEvent, ScanReport } from '~/types/sysdll'

const fakeReport: ScanReport = {
  targets: [],
  files: [],
  total_files: 42,
  parsed_files: 40,
  failed_files: 2,
  duration_ms: 100,
}

beforeEach(() => {
  setActivePinia(createPinia())
})

describe('useRepairStore', () => {
  it('starts with empty state', () => {
    const store = useRepairStore()
    expect(store.logs).toEqual([])
    expect(store.cliRunning).toBe(false)
    expect(store.lastFixResult).toBeNull()
    expect(store.scanProgress).toEqual({ scanned: 0, total: 0 })
  })

  it('appends log lines from log events', () => {
    const store = useRepairStore()
    const evt: CliEvent = { event: 'log', level: 'info', message: 'scanning C:\\System32' }
    store.appendLog(evt)
    expect(store.logs.length).toBe(1)
    expect(store.logs[0]?.message).toContain('scanning')
    expect(store.logs[0]?.level).toBe('info')
  })

  it('treats error events as error level', () => {
    const store = useRepairStore()
    store.appendLog({ event: 'error', message: 'broken' })
    expect(store.logs[0]?.level).toBe('error')
  })

  it('records fix_done results', () => {
    const store = useRepairStore()
    store.appendLog({ event: 'fix_done', target: 'C:\\foo.dll', backup: 'C:\\backup\\foo.dll.bak' })
    expect(store.lastFixResult?.target).toBe('C:\\foo.dll')
    expect(store.lastFixResult?.backup).toContain('foo.dll.bak')
  })

  it('treats restore_done as a regular info line', () => {
    const store = useRepairStore()
    store.appendLog({ event: 'restore_done', target: 'C:\\foo.dll', backup: 'C:\\backup\\foo.dll.bak' })
    expect(store.logs[0]?.message).toContain('restored C:\\foo.dll')
  })

  it('captures progress events into scanProgress', () => {
    const store = useRepairStore()
    store.appendLog({ event: 'progress', scanned: 5, total: 10, current: 'a.dll' })
    expect(store.scanProgress).toEqual({ scanned: 5, total: 10, current: 'a.dll' })
  })

  it('captures scan_done into the log with file counts', () => {
    const store = useRepairStore()
    store.appendLog({ event: 'scan_done', report: fakeReport })
    expect(store.logs[0]?.message).toContain('42 files')
  })

  it('exit event clears cliRunning', () => {
    const store = useRepairStore()
    store.cliRunning = true
    store.appendLog({ event: 'exit', code: 0 })
    expect(store.cliRunning).toBe(false)
    expect(store.logs[0]?.level).toBe('warn')
  })

  it('caps the log buffer at 500 lines', () => {
    const store = useRepairStore()
    for (let i = 0; i < 600; i++) {
      store.appendLog({ event: 'log', level: 'info', message: `line ${i}` })
    }
    expect(store.logs.length).toBe(500)
    expect(store.logs[store.logs.length - 1]?.message).toBe('line 599')
  })

  it('clears all state', () => {
    const store = useRepairStore()
    store.appendLog({ event: 'log', level: 'info', message: 'x' })
    store.appendLog({ event: 'progress', scanned: 5, total: 10 })
    store.clearLogs()
    expect(store.logs).toEqual([])
    expect(store.lastFixResult).toBeNull()
    expect(store.scanProgress).toEqual({ scanned: 0, total: 0 })
  })
})
