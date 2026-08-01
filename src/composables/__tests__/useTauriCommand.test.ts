import { describe, expect, it, vi } from 'vitest'

// Mock the @tauri-apps/api/core module before importing the composable so the
// wrapper sees the mocked `invoke`.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { invoke } from '@tauri-apps/api/core'
import { useTauriCommand } from '../useTauriCommand'

describe('useTauriCommand', () => {
  it('exposes loading and error transitions around invoke', async () => {
    const mocked = vi.mocked(invoke)
    mocked.mockResolvedValueOnce({ ok: true })

    const cmd = useTauriCommand<[{ id: number }], { ok: boolean }>('fake_cmd')
    expect(cmd.loading.value).toBe(false)
    expect(cmd.data.value).toBeNull()
    expect(cmd.error.value).toBeNull()

    const promise = cmd.run({ id: 1 })
    expect(cmd.loading.value).toBe(true)
    const result = await promise

    expect(result).toEqual({ ok: true })
    expect(cmd.loading.value).toBe(false)
    expect(cmd.data.value).toEqual({ ok: true })
    expect(cmd.error.value).toBeNull()
  })

  it('captures rejection as an error string', async () => {
    const mocked = vi.mocked(invoke)
    mocked.mockRejectedValueOnce(new Error('rip'))

    const cmd = useTauriCommand<[], string>('fake_cmd')
    await cmd.run()

    expect(cmd.error.value).toBe('Error: rip')
    expect(cmd.data.value).toBeNull()
    expect(cmd.loading.value).toBe(false)
  })
})
