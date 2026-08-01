import { beforeEach, describe, expect, it } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useRepairStore } from '../repair'
import type { CliEvent } from '~/types/sysdll'

beforeEach(() => {
  setActivePinia(createPinia())
})

describe('useRepairStore', () => {
  it('starts with empty state', () => {
    const store = useRepairStore()
    expect(store.logs).toEqual([])
    expect(store.cliRunning).toBe(false)
    expect(store.lastFixResult).toBeNull()
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
    store.clearLogs()
    expect(store.logs).toEqual([])
    expect(store.lastFixResult).toBeNull()
  })
})
