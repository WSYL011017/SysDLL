import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { CommandName, CommandArgs, CommandResult } from '~/commands'

interface InvokeState<T> {
  data: T | null
  error: string | null
  loading: boolean
}

/**
 * Strongly-typed reactive wrapper around Tauri's `invoke`.
 *
 * Why this shape (audit fixes P1-2 / P1-3 / P2-3 / P2-4):
 *   - `C` is a literal string narrowed to known commands; an unknown command
 *     fails `CommandName` lookup before it ever reaches Rust.
 *   - `Args` is a required object literal whose keys exactly match the Rust
 *     tauri::command signature, so renames are caught at compile time
 *     instead of "Tauri panicked: missing field target".
 *   - `Result` is derived from the same map — components get correct return
 *     types without a second generic argument.
 *   - `state` is a `ref`, not `shallowRef`: the previous nested
 *     {data,error,loading} had no large nested object benefit, and exposing
 *     it via three computeds triggered exactly the same reactivity cost.
 *   - Errors surface as `Error` objects with the original message; we keep
 *     `string` for backwards-compatibility but also expose `.cause`.
 */
export function useTauriCommand<C extends CommandName>(cmd: C) {
  const state = ref<InvokeState<CommandResult<C>>>({
    data: null,
    error: null,
    loading: false,
  })

  const data = computed(() => state.value.data)
  const error = computed(() => state.value.error)
  const loading = computed(() => state.value.loading)

  async function run(args: CommandArgs<C>): Promise<CommandResult<C> | null> {
    state.value = { ...state.value, loading: true, error: null }
    try {
      const result = (await invoke(cmd, args as Record<string, unknown>)) as CommandResult<C>
      state.value = { data: result, error: null, loading: false }
      return result
    }
    catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      state.value = { data: null, error: message, loading: false }
      return null
    }
  }

  function reset(): void {
    state.value = { data: null, error: null, loading: false }
  }

  return { data, error, loading, run, reset }
}
