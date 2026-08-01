import { onScopeDispose } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { CliEvent } from '~/types/sysdll'

/**
 * Subscribe to the elevated CLI child's output stream.
 * Returns a cleanup function (also auto-disposed when the calling scope ends).
 */
export function useCliStream(handler: (evt: CliEvent) => void) {
  const unlistens: UnlistenFn[] = []

  void (async () => {
    const u = await listen<string>('sysdll://cli-event', (e) => {
      try {
        const parsed = JSON.parse(e.payload) as CliEvent
        handler(parsed)
      }
      catch {
        // pass raw string through so the UI can still show it
        handler({ event: 'raw', line: e.payload })
      }
    })
    unlistens.push(u)
  })()

  onScopeDispose(() => {
    unlistens.forEach(u => u())
  })

  return () => unlistens.forEach(u => u())
}
