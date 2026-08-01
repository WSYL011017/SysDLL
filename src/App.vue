<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useDark } from '@vueuse/core'
import { useScanStore } from '~/stores/scan'
import { useRepairStore } from '~/stores/repair'
import { useCliStream } from '~/composables/useCliStream'
import { useTauriCommand } from '~/composables/useTauriCommand'
import ScanTargetPicker from '~/components/scan/ScanTargetPicker.vue'
import ScanProgress from '~/components/scan/ScanProgress.vue'
import IssueList from '~/components/diagnose/IssueList.vue'
import IssueDetail from '~/components/diagnose/IssueDetail.vue'
import RepairLog from '~/components/repair/RepairLog.vue'
import { t } from '~/i18n/zh-CN'

useDark({
  selector: 'html',
  attribute: 'class',
  valueDark: 'dark',
})

const scanStore = useScanStore()
const repairStore = useRepairStore()
const selected = ref<string | null>(null)

useCliStream((evt) => {
  repairStore.appendLog(evt)
})

const launchCmd = useTauriCommand<[], number>('launch_cli')

async function startRepair() {
  repairStore.cliRunning = true
  await launchCmd.run()
}

watch(() => scanStore.diagnostics, (diags) => {
  if (!selected.value && diags.length) {
    selected.value = diags[0]!.subject
  }
})

const selectedDiagnostic = computed(() => {
  if (!selected.value) return null
  return scanStore.diagnostics.find(d => d.subject === selected.value) ?? null
})

const stats = computed(() => scanStore.severityCount)
</script>

<template>
  <div class="h-full flex flex-col bg-base text-base">
    <header class="border-b border-base px-4 py-2 flex items-center justify-between">
      <div class="flex items-center gap-3">
        <div class="i-ph-stack-duotone text-primary-600 text-xl" />
        <div>
          <h1 class="text-sm font-semibold tracking-tight">
            SysDLL 修复工具
          </h1>
          <p class="text-xs color-mute">
            系统 DLL 扫描与修复
          </p>
        </div>
      </div>
      <div class="flex items-center gap-4">
        <div class="hidden md:flex items-center gap-3 text-xs color-mute mono">
          <span><span class="text-red-500">{{ stats.critical }}</span> {{ t.app.statCritical }}</span>
          <span><span class="text-orange-500">{{ stats.error }}</span> {{ t.app.statError }}</span>
          <span><span class="text-yellow-500">{{ stats.warning }}</span> {{ t.app.statWarn }}</span>
          <span>{{ stats.info }} {{ t.app.statInfo }}</span>
        </div>
        <button
          class="btn-primary"
          :disabled="repairStore.cliRunning"
          @click="startRepair"
        >
          <div class="i-ph-shield-check-duotone" />
          {{ t.app.startRepair }}
        </button>
      </div>
    </header>

    <main class="flex-1 grid grid-cols-12 gap-0 min-h-0">
      <!-- 左侧：扫描控制 -->
      <aside class="col-span-3 border-r border-base p-4 flex flex-col gap-4 overflow-auto">
        <ScanTargetPicker @scan="scanStore.runScan" />
        <ScanProgress v-if="scanStore.scanning || scanStore.progress.total" />
      </aside>

      <!-- 中部：问题列表 -->
      <section class="col-span-4 border-r border-base flex flex-col min-h-0">
        <div class="px-4 py-2 border-b border-base text-xs color-mute uppercase tracking-wide">
          {{ t.app.issuesHeading(scanStore.issueCount) }}
        </div>
        <div class="flex-1 overflow-auto">
          <IssueList
            :diagnostics="scanStore.diagnostics"
            :selected="selected"
            @select="selected = $event"
          />
        </div>
      </section>

      <!-- 右侧：详情 + 日志 -->
      <section class="col-span-5 flex flex-col min-h-0">
        <div class="flex-1 overflow-auto border-b border-base">
          <IssueDetail v-if="selectedDiagnostic" :diagnostic="selectedDiagnostic" />
          <div v-else class="h-full flex items-center justify-center color-mute text-sm">
            {{ t.app.selectIssueHint }}
          </div>
        </div>
        <div class="h-64 flex flex-col">
          <div class="px-4 py-2 border-b border-base text-xs color-mute uppercase tracking-wide flex items-center justify-between">
            <span>{{ t.app.repairLogHeading }}</span>
            <button class="text-xs color-mute hover:color-base" @click="repairStore.clearLogs">
              {{ t.app.clearLog }}
            </button>
          </div>
          <RepairLog :lines="repairStore.logs" />
        </div>
      </section>
    </main>
  </div>
</template>