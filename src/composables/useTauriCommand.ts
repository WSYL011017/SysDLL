import { computed, ref, shallowRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface InvokeState<T> {
  data: T | null
  error: string | null
  loading: boolean
}

/**
 * Thin reactive wrapper around Tauri's `invoke`. Tracks loading + error so
 * components can render skeletons and inline error states without each call
 * having to write the same boilerplate.
 */
export function useTauriCommand<Args extends unknown[], Result>(
  cmd: string,
) {
  const state = shallowRef<InvokeState<Result>>({
    data: null,
    error: null,
    loading: false,
  })

  const data = computed(() => state.value.data)
  const error = computed(() => state.value.error)
  const loading = computed(() => state.value.loading)

  async function run(...args: Args): Promise<Result | null> {
    state.value = { ...state.value, loading: true, error: null }
    try {
      const result = (await invoke(cmd, argsToObject(cmd, args))) as Result
      state.value = { data: result, error: null, loading: false }
      return result
    }
    catch (err) {
      state.value = { data: null, error: String(err), loading: false }
      return null
    }
  }

  return { data, error, loading, run }
}

/**
 * Tauri expects a single object argument; positional args become keys derived
 * from the command name. For example `invoke('scan_targets', { targets })`
 * -> `run(targets)`. We accept a custom mapper because some commands take
 * multiple positional args (e.g. `fix { target, source }`).
 */
function argsToObject(cmd: string, args: unknown[]): Record<string, unknown> {
  const KEY_MAP: Record<string, string[]> = {
    scan_targets: ['targets'],
    run_diagnostics_cmd: [],
    launch_cli: [],
    list_backups: [],
    restore_backup: ['target'],
    ping: [],
  }
  const keys = KEY_MAP[cmd]
  if (!keys || keys.length === 0) {
    return {}
  }
  const obj: Record<string, unknown> = {}
  keys.forEach((key, i) => {
    obj[key] = args[i]
  })
  return obj
}

/**
 * Shared "currently selected" state for the dashboard sidebar.
 * Centralized here so multiple components can stay in sync without prop drilling.
 */
export const selectedDiagnosticSubject = ref<string | null>(null)
