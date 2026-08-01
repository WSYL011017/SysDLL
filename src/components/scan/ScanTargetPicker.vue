<script setup lang="ts">
import { ref } from 'vue'
import { useScanStore } from '~/stores/scan'
import type { ScanTarget } from '~/types/sysdll'

const emit = defineEmits<{ scan: [targets: ScanTarget[]] }>()

const scanStore = useScanStore()

// MVP default: the two Windows system DLL directories. Users can add custom paths.
const defaultTargets: ScanTarget[] = [
  { path: 'C:\\Windows\\System32', recursive: true },
  { path: 'C:\\Windows\\SysWOW64', recursive: true },
]
const extraPaths = ref<string>('')

function buildTargets(): ScanTarget[] {
  const extras = extraPaths.value
    .split('\n')
    .map(s => s.trim())
    .filter(Boolean)
    .map(p => ({ path: p, recursive: true }))
  return [...defaultTargets, ...extras]
}

function startScan() {
  emit('scan', buildTargets())
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <section>
      <h2 class="text-xs uppercase tracking-wide color-mute mb-2">
        Default targets
      </h2>
      <ul class="flex flex-col gap-1 mono text-xs">
        <li
          v-for="t in defaultTargets"
          :key="t.path"
          class="flex items-center gap-2 px-2 py-1 rounded bg-secondary"
        >
          <div class="i-ph-folder-duotone text-primary-600" />
          <span class="truncate flex-1" :title="t.path">{{ t.path }}</span>
          <span class="color-mute">recursive</span>
        </li>
      </ul>
    </section>

    <section>
      <h2 class="text-xs uppercase tracking-wide color-mute mb-2">
        Extra paths (one per line)
      </h2>
      <textarea
        v-model="extraPaths"
        rows="4"
        class="w-full mono text-xs p-2 rounded border border-base bg-base focus:outline-none focus:border-active resize-y"
        placeholder="C:\Program Files\MyApp"
      />
    </section>

    <button
      class="btn-primary w-full justify-center"
      :disabled="scanStore.scanning"
      @click="startScan"
    >
      <div v-if="scanStore.scanning" class="i-ph-spinner-gap-duotone animate-spin" />
      <div v-else class="i-play_circle-fill-duotone" />
      {{ scanStore.scanning ? 'Scanning…' : 'Run scan' }}
    </button>

    <div v-if="scanStore.report" class="text-xs color-mute mono leading-relaxed">
      <div>
        scanned: <span class="color-base">{{ scanStore.report.total_files }}</span>
        parsed: <span class="color-base">{{ scanStore.report.parsed_files }}</span>
        failed: <span class="color-base">{{ scanStore.report.failed_files }}</span>
      </div>
      <div>
        duration: <span class="color-base">{{ scanStore.report.duration_ms }} ms</span>
      </div>
      <div v-if="scanStore.scanError" class="text-red-500 mt-2">
        {{ scanStore.scanError }}
      </div>
    </div>
  </div>
</template>
