<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '~/stores/scan'

const scanStore = useScanStore()

const percent = computed(() => {
  const total = scanStore.progress.total
  if (!total) return 0
  return Math.round((scanStore.progress.scanned / total) * 100)
})
</script>

<template>
  <div class="flex flex-col gap-1">
    <div class="text-xs color-mute mono flex justify-between">
      <span>scanning {{ scanStore.progress.scanned }} / {{ scanStore.progress.total }}</span>
      <span>{{ percent }}%</span>
    </div>
    <div class="h-1 bg-secondary rounded overflow-hidden">
      <div
        class="h-full bg-primary-500 transition-all"
        :style="{ width: `${percent}%` }"
      />
    </div>
  </div>
</template>
