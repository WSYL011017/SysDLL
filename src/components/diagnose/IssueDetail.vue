<script setup lang="ts">
import { computed } from 'vue'
import { useTauriCommand } from '~/composables/useTauriCommand'
import type { Diagnostic } from '~/types/sysdll'
import { t } from '~/i18n/zh-CN'

const props = defineProps<{
  diagnostic: Diagnostic
}>()

const SEVERITY_BADGE: Record<Diagnostic['severity'], string> = {
  critical: 'bg-red-500/10 text-red-500 border-red-500/30',
  error: 'bg-orange-500/10 text-orange-500 border-orange-500/30',
  warning: 'bg-yellow-500/10 text-yellow-500 border-yellow-500/30',
  info: 'bg-blue-500/10 text-blue-500 border-blue-500/30',
}

const dependents = computed(() => props.diagnostic.dependents.slice(0, 20))
const overflow = computed(() => Math.max(0, props.diagnostic.dependents.length - 20))

const restoreCmd = useTauriCommand('restore_backup')

// Audit-fix P1-12 / P2-14: the restore button is only enabled when the
//   diagnostic has a concrete target on disk (anything else would silently
//   fail at the Rust side). The Microsoft download placeholder stays
//   disabled — it's a follow-up.
const canRestore = computed(() => props.diagnostic.related_paths.length > 0)

async function restoreBackup(): Promise<void> {
  const target = props.diagnostic.related_paths[0]
  if (!target) return
  const ok = confirm(t.issueDetail.restoreConfirm(target))
  if (!ok) return
  await restoreCmd.run({ target: String(target) })
}
</script>

<template>
  <div class="p-4 flex flex-col gap-4">
    <header class="flex items-start gap-3">
      <span
        class="px-2 py-0.5 text-xs uppercase rounded border"
        :class="SEVERITY_BADGE[diagnostic.severity]"
      >
        {{ t.severity[diagnostic.severity] }}
      </span>
      <div class="flex-1 min-w-0">
        <h2 class="text-base font-semibold">
          {{ diagnostic.title }}
        </h2>
        <p class="text-sm color-mute mt-1">
          {{ diagnostic.detail }}
        </p>
      </div>
    </header>

    <section>
      <h3 class="text-xs uppercase tracking-wide color-mute mb-1">
        {{ t.issueDetail.subject }}
      </h3>
      <div class="mono text-sm px-3 py-2 rounded bg-secondary break-all">
        {{ diagnostic.subject }}
      </div>
    </section>

    <section v-if="diagnostic.related_paths.length">
      <h3 class="text-xs uppercase tracking-wide color-mute mb-1">
        {{ t.issueDetail.relatedFiles }}
      </h3>
      <ul class="flex flex-col gap-1">
        <li
          v-for="p in diagnostic.related_paths"
          :key="p"
          class="mono text-xs px-2 py-1 rounded bg-secondary truncate"
          :title="p"
        >
          {{ p }}
        </li>
      </ul>
    </section>

    <section v-if="dependents.length">
      <h3 class="text-xs uppercase tracking-wide color-mute mb-1">
        {{ t.issueDetail.dependents }}
      </h3>
      <div class="flex flex-wrap gap-1.5">
        <span
          v-for="dep in dependents"
          :key="dep"
          class="mono text-xs px-2 py-0.5 rounded bg-secondary"
        >
          {{ dep }}
        </span>
        <span v-if="overflow" class="text-xs color-mute">
          {{ t.issueDetail.moreDependents(overflow) }}
        </span>
      </div>
    </section>

    <section class="border-t border-base pt-3 flex gap-2">
      <button class="btn-action" disabled title="尚未实现">
        <div class="i-ph-download-simple-duotone" />
        {{ t.issueDetail.downloadMicrosoft }}
      </button>
      <button
        class="btn-action"
        :disabled="!canRestore"
        :title="canRestore ? '' : '需要先有相关文件路径'"
        @click="restoreBackup"
      >
        <div class="i-ph-floppy-disk-back-duotone" />
        {{ t.issueDetail.restoreBackup }}
      </button>
    </section>
  </div>
</template>
