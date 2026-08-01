<script setup lang="ts">
import type { Diagnostic, Severity } from '~/types/sysdll'
import { t } from '~/i18n/zh-CN'

defineProps<{
  diagnostics: Diagnostic[]
  selected: string | null
}>()

defineEmits<{
  select: [subject: string]
}>()

const SEVERITY_COLOR: Record<Severity, string> = {
  critical: 'bg-red-500',
  error: 'bg-orange-500',
  warning: 'bg-yellow-500',
  info: 'bg-blue-500',
}

// data contract stays English; render label is localized via t.kind
const KIND_ICON: Record<Diagnostic['kind'], string> = {
  missing_dll: 'i-ph-warning-circle-duotone text-red-500',
  orphan_dll: 'i-ph-question-duotone text-blue-500',
  broken_pe_parse: 'i-ph-file-x-duotone text-orange-500',
  circular_dependency: 'i-ph-arrows-clockwise-duotone text-yellow-500',
  parse_failure: 'i-ph-x-circle-duotone text-orange-500',
}
</script>

<template>
  <ul class="divide-y divide-base">
    <li
      v-for="d in diagnostics"
      :key="`${d.kind}-${d.subject}`"
      class="px-4 py-2 cursor-pointer hover:bg-active transition-colors"
      :class="{ 'bg-active': selected === d.subject }"
      @click="$emit('select', d.subject)"
    >
      <div class="flex items-start gap-2">
        <div :class="KIND_ICON[d.kind]" class="text-lg mt-0.5 shrink-0" />
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span class="w-1.5 h-1.5 rounded-full" :class="SEVERITY_COLOR[d.severity]" />
            <span class="text-sm truncate" :title="d.title">{{ d.title }}</span>
          </div>
          <div class="text-xs color-mute mt-1 line-clamp-2" :title="d.detail">
            {{ d.detail }}
          </div>
          <div v-if="d.dependents.length" class="text-xs color-mute mono mt-1">
            · {{ t.issueList.affects(d.dependents.length) }}
          </div>
        </div>
      </div>
    </li>
  </ul>
  <div v-if="!diagnostics.length" class="p-6 text-center color-mute text-sm">
    {{ t.issueList.emptyHint }}
  </div>
</template>