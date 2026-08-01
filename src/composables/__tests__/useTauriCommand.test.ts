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
    mocked.mockResolvedValueOnce('pong')

    const cmd = useTauriCommand('ping')
    expect(cmd.loading.value).toBe(false)
    expect(cmd.data.value).toBeNull()
    expect(cmd.error.value).toBeNull()

    const promise = cmd.run({})
    expect(cmd.loading.value).toBe(true)
    const result = await promise

    expect(result).toBe('pong')
    expect(cmd.loading.value).toBe(false)
    expect(cmd.data.value).toBe('pong')
    expect(cmd.error.value).toBeNull()

    // Sanity-check the call signature passes the empty arg object through
    expect(mocked).toHaveBeenCalledWith('ping', {})
  })

  it('captures rejection as a plain message string', async () => {
    const mocked = vi.mocked(invoke)
    mocked.mockRejectedValueOnce(new Error('rip'))

    const cmd = useTauriCommand('ping')
    await cmd.run({})

    expect(cmd.error.value).toBe('rip')
    expect(cmd.data.value).toBeNull()
    expect(cmd.loading.value).toBe(false)
  })

  it('reset() clears the cached data', async () => {
    const mocked = vi.mocked(invoke)
    mocked.mockResolvedValueOnce('pong')

    const cmd = useTauriCommand('ping')
    await cmd.run({})
    expect(cmd.data.value).toBe('pong')

    cmd.reset()
    expect(cmd.data.value).toBeNull()
    expect(cmd.error.value).toBeNull()
    expect(cmd.loading.value).toBe(false)
  })
})
