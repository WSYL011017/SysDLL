<script setup lang="ts">
import { computed, ref, watch, nextTick } from 'vue'
import type { LogLine } from '~/stores/repair'
import { t } from '~/i18n/zh-CN'

const props = defineProps<{
  lines: LogLine[]
}>()

const container = ref<HTMLElement | null>(null)

const formattedLines = computed(() =>
  props.lines.map(l => ({
    key: `${l.at}-${l.message}`,
    at: new Date(l.at).toLocaleTimeString(),
    level: l.level,
    message: l.message,
  })),
)

watch(() => props.lines.length, async () => {
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
    <div v-if="!lines.length" class="color-mute">
      {{ t.repairLog.emptyHint }}
    </div>
    <div
      v-for="line in formattedLines"
      :key="line.key"
      :class="{
        'text-red-400': line.level === 'error',
        'text-yellow-300': line.level === 'warn',
        'text-green-300': line.level === 'info',
      }"
    >
      <span class="color-mute">[{{ line.at }}]</span> {{ line.message }}
    </div>
  </div>
</template>
