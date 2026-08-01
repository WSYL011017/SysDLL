<script setup lang="ts">
import { computed, ref, watch, nextTick } from 'vue'
import type { LogLine } from '~/stores/repair'
import { t } from '~/i18n/zh-CN'

const props = defineProps<{
  lines: LogLine[]
}>()

const container = ref<HTMLElement | null>(null)

const text = computed(() =>
  props.lines
    .map(l => `[${new Date(l.at).toLocaleTimeString()}] ${l.message}`)
    .join('\n'),
)

watch(text, async () => {
  await nextTick()
  if (container.value) {
    container.value.scrollTop = container.value.scrollHeight
  }
})
</script>

<template>
  <div
    ref="container"
    class="flex-1 overflow-auto bg-#0a0a0a dark:bg-#0a0a0a text-#cdd6f4 font-mono text-xs leading-relaxed p-3 whitespace-pre-wrap"
  >
    <span v-if="!lines.length" class="color-mute">{{ t.repairLog.emptyHint }}</span>
    <span
      v-for="(line, i) in lines"
      :key="i"
      :class="{
        'text-red-400': line.level === 'error',
        'text-yellow-300': line.level === 'warn',
        'text-green-300': line.level === 'info',
      }"
    >
      {{ line.message }}
    </span>
  </div>
</template>