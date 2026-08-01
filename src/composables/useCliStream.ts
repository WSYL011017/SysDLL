import { onScopeDispose } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { t } from '~/i18n/zh-CN'
import type { CliEvent } from '~/types/sysdll'

/**
 * Subscribe to the elevated CLI child's output stream.
 * Returns a cleanup function (also auto-disposed when the calling scope ends).
 *
 * Audit fixes P2-1 / P2-2 / P1-8:
 *   - Single UnlistenFn (the array was always length 1).
 *   - Promise is registered eagerly and awaited *before* the onScopeDispose
 *     callback is registered, so a fast unmount can't run the cleanup
 *     while the listen() future is still pending.
 *   - Unknown / malformed events surface as `LogLevel.error` lines instead
 *     of being silently dropped.
 */
export function useCliStream(handler: (evt: CliEvent) => void): () => void {
  let unlisten: UnlistenFn | null = null
  let cancelled = false

  const setup = (async () => {
    try {
      const u = await listen<string>('sysdll://cli-event', (e) => {
        // The Rust side serialises every event via serde_json so any
        //   non-JSON payload we get here is a genuine surprise.
        let parsed: CliEvent | null = null
        try {
          parsed = JSON.parse(e.payload) as CliEvent
        }
        catch {
          parsed = null
        }
        if (parsed) {
          handler(parsed)
          return
        }
        // Fallback: surface the raw payload as an error log so the user
        // never loses data; App.vue can still render it.
        handler({
          event: 'log',
          level: 'error',
          message: t.repairLog.unknownEvent(e.payload),
        })
      })
      // Audit-fix P2-1: if the scope was torn down while we awaited,
      //   immediately drop the listener rather than leak it.
      if (cancelled) {
        u()
      }
      else {
        unlisten = u
      }
    }
    catch (err) {
      // listen() itself failed (e.g. outside of a Tauri context). Surface
      // a single error log so the UI shows something.
      handler({
        event: 'log',
        level: 'error',
        message: `cli stream listen failed: ${String(err)}`,
      })
    }
  })()

  void setup

  const cleanup = () => {
    cancelled = true
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  }

  onScopeDispose(cleanup)
  return cleanup
}
