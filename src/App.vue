<template>
  <div class="h-screen flex overflow-hidden">
    <!-- 侧栏（玻璃拟态） -->
    <aside class="glass flex flex-col border-r border-slate-200/60 shrink-0 transition-all duration-300 z-20"
           :class="collapsed ? 'w-[68px]' : 'w-52'">
      <div class="flex items-center gap-2.5 h-16 px-4 shrink-0">
        <div class="w-9 h-9 rounded-xl bg-gradient-to-br from-primary-600 via-accent-500 to-flare-500 shadow-glow-sm flex items-center justify-center text-white shrink-0 rotate-3">
          <Receipt :size="18" />
        </div>
        <div v-if="!collapsed" class="min-w-0">
          <p class="font-display text-sm font-bold text-slate-800 leading-tight truncate">发票报销助手</p>
          <p class="text-[11px] text-slate-400 leading-tight">v{{ version }}</p>
        </div>
      </div>

      <nav class="flex-1 py-3 space-y-1 px-3">
        <router-link v-for="item in navItems" :key="item.to" :to="item.to" :title="item.label"
                     class="group flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition-all duration-200"
                     :class="navLinkClass(item.to)">
          <component :is="item.icon" :size="18" class="shrink-0 transition-transform duration-200 group-hover:scale-110" />
          <span v-if="!collapsed" class="truncate">{{ item.label }}</span>
          <span v-if="!collapsed && item.badge" class="ml-auto min-w-5 h-5 px-1.5 rounded-full text-[11px] font-semibold flex items-center justify-center"
                :class="item.badge > 0 ? 'bg-primary-600 text-white' : 'bg-slate-200 text-slate-400'">
            {{ item.badge > 99 ? '99+' : item.badge }}
          </span>
        </router-link>
      </nav>

      <!-- 设置与调试入口（左下角小入口） -->
      <div class="p-3 border-t border-slate-200/60 space-y-1">
        <router-link to="/settings" title="报销标准"
                     class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs text-slate-400 hover:text-slate-600 hover:bg-slate-100/80 transition-colors">
          <Settings :size="13" class="shrink-0" />
          <span v-if="!collapsed">报销标准</span>
        </router-link>
        <router-link to="/debug" title="调试工具"
                     class="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs text-slate-400 hover:text-slate-600 hover:bg-slate-100/80 transition-colors">
          <Bug :size="13" class="shrink-0" />
          <span v-if="!collapsed">调试工具</span>
        </router-link>
      </div>
    </aside>

    <!-- 主区 -->
    <div class="flex-1 flex flex-col min-w-0">
      <!-- 头部 -->
      <header class="glass flex items-center justify-between h-16 px-5 border-b border-slate-200/60 shrink-0 z-10">
        <div class="flex items-center gap-3 min-w-0">
          <button @click="toggleCollapse" class="w-9 h-9 rounded-xl text-slate-400 hover:text-primary-600 hover:bg-white transition-all flex items-center justify-center"
                  :aria-label="collapsed ? '展开侧栏' : '收起侧栏'" :title="collapsed ? '展开侧栏' : '收起侧栏'">
            <PanelLeftClose v-if="!collapsed" :size="17" />
            <PanelLeftOpen v-else :size="17" />
          </button>
          <h1 class="font-display text-lg font-bold text-slate-800 truncate">{{ pageTitle }}</h1>
        </div>
        <div class="flex items-center gap-2.5 shrink-0">
          <!-- OCR 状态小芯片 -->
          <span class="chip border shadow-card" :class="ocrOnline ? 'bg-emerald-50 text-emerald-700 border-emerald-200/70' : 'bg-rose-50 text-rose-700 border-rose-200/70'">
            <span class="w-1.5 h-1.5 rounded-full" :class="ocrOnline ? 'bg-emerald-500 animate-pulse-soft' : 'bg-rose-500'" />
            OCR {{ ocrOnline ? '在线' : '离线' }}
          </span>
          <!-- 检查更新 -->
          <button @click="onCheckClick" title="检查更新" :aria-label="'检查更新'"
                  class="relative w-9 h-9 rounded-xl text-slate-400 hover:text-primary-600 hover:bg-white transition-all flex items-center justify-center">
            <RefreshCw v-if="checking" :size="17" class="animate-spin" />
            <Download v-else :size="17" />
            <span v-if="hasUpdate" class="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-rose-500 ring-2 ring-white" />
          </button>
          <!-- GitHub 仓库 -->
          <a href="https://github.com/chenfuxu920/invoice-reimbursement" title="GitHub 仓库"
             class="w-9 h-9 rounded-xl text-slate-400 hover:text-primary-600 hover:bg-white transition-all flex items-center justify-center"
             @click.prevent="openUrl('https://github.com/chenfuxu920/invoice-reimbursement')">
            <Github :size="17" />
          </a>
        </div>
      </header>

      <!-- 流程轨道（设置/调试页不显示） -->
      <div v-if="!['/settings', '/debug'].includes(route.path)" class="px-5 pt-3 shrink-0">
        <AppStepper />
      </div>

      <!-- 主内容 -->
      <main class="flex-1 overflow-auto">
        <router-view v-slot="{ Component }">
          <Transition name="route-fade" mode="out-in">
            <component :is="Component" />
          </Transition>
        </router-view>
      </main>
    </div>

    <!-- 更新对话框 -->
    <Teleport to="body">
      <Transition name="modal">
        <div v-if="dialogOpen" class="fixed inset-0 z-[70] flex items-center justify-center p-6">
          <div class="absolute inset-0 bg-slate-900/25 backdrop-blur-[2px]" @click="closeDialog" />
          <div class="relative w-full max-w-md glass rounded-2xl border-slate-200/60 shadow-card-lg p-5 animate-scale-in">
            <div class="flex items-start gap-3.5">
              <div class="w-11 h-11 rounded-xl bg-gradient-to-br from-primary-600 via-accent-500 to-flare-500 shadow-glow-sm flex items-center justify-center text-white shrink-0 rotate-3">
                <Download :size="20" />
              </div>
              <div class="min-w-0 flex-1 pt-0.5">
                <p class="font-display text-base font-bold text-slate-800 leading-tight">发现新版本 v{{ updateVersion }}</p>
                <p v-if="!update?.body" class="text-xs text-slate-400 mt-1">{{ update?.date ?? '有新版本可用' }}</p>
              </div>
              <button class="text-slate-400 hover:text-slate-600 transition-colors shrink-0 disabled:opacity-40"
                      :disabled="downloading" :aria-label="'关闭'" title="关闭" @click="closeDialog">
                <X :size="16" />
              </button>
            </div>

            <!-- 更新说明 -->
            <div v-if="update?.body" class="mt-3.5 max-h-36 overflow-y-auto rounded-xl bg-slate-50/80 border border-slate-200/60 px-3.5 py-2.5 text-[13px] leading-relaxed text-slate-600 whitespace-pre-wrap">
              {{ update.body }}
            </div>

            <!-- 下载进度 -->
            <div v-if="downloading && !downloadReady" class="mt-4">
              <div class="flex items-center justify-between text-xs text-slate-500 mb-1.5">
                <span>正在下载更新…</span>
                <span class="font-semibold text-primary-600">{{ progressPercent }}%</span>
              </div>
              <div class="h-1.5 rounded-full bg-slate-200/80 overflow-hidden">
                <div class="h-full rounded-full bg-gradient-to-r from-primary-600 to-accent-500 transition-all duration-200"
                     :style="{ width: progressPercent + '%' }" />
              </div>
            </div>

            <!-- 下载完成（Windows 安装器随即运行并退出） -->
            <div v-else-if="downloadReady" class="mt-4 flex items-center gap-2 text-sm text-emerald-700">
              <Loader2 :size="15" class="animate-spin" />
              下载完成，即将安装重启…
            </div>

            <!-- 下载失败 -->
            <div v-else-if="downloadError" class="mt-4 rounded-xl bg-rose-50 border border-rose-200/70 px-3.5 py-2.5 text-[13px] text-rose-600 leading-relaxed break-words">
              下载失败：{{ downloadError }}
            </div>

            <!-- 操作按钮 -->
            <div class="mt-4 flex items-center justify-end gap-2.5">
              <template v-if="downloadError">
                <button class="btn-primary-glow px-5 py-2 text-sm" @click="startDownload">重试</button>
                <button class="px-3 py-2 text-sm text-slate-500 hover:text-slate-700 rounded-xl transition-colors" @click="closeDialog">稍后再说</button>
              </template>
              <template v-else-if="downloadReady">
                <button class="btn-primary-glow px-5 py-2 text-sm" @click="relaunchNow">{{ isPortable ? '立即安装' : '立即重启' }}</button>
              </template>
              <template v-else>
                <button class="btn-primary-glow px-5 py-2 text-sm" :disabled="downloading" @click="startDownload">立即更新</button>
                <button class="px-3 py-2 text-sm text-slate-500 hover:text-slate-700 rounded-xl transition-colors disabled:opacity-40"
                        :disabled="downloading" @click="closeDialog">稍后再说</button>
              </template>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <AppToast />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useEventListener } from '@vueuse/core'
import {
  PanelLeftClose, PanelLeftOpen, Home, Upload, Link2, Download, Bug, Settings, Receipt, Github, RefreshCw, X, Loader2,
} from 'lucide-vue-next'
import AppStepper from './components/ui/AppStepper.vue'
import AppToast from './components/ui/AppToast.vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { toast } from './composables/toast'
import { useOcrStatus, initOcrStatus } from './composables/ocr'
import { useInvoiceStore } from './stores/invoice'
import { useMatchStore } from './stores/match'
import pkg from '../package.json'

const version = pkg.version
const route = useRoute()
const invoiceStore = useInvoiceStore()
const matchStore = useMatchStore()
const { ocrOnline } = useOcrStatus()

// ── 应用更新（便携版走自制命令，安装版走插件，检查时自动分叉） ──
interface PortableUpdateInfo {
  latest_version: string
  has_update: boolean
  download_url: string
  signature_url: string
}

const update = ref<Update | null>(null) // 插件路径（安装版）
const portableInfo = ref<PortableUpdateInfo | null>(null) // 自制命令（便携版）
const installedPath = ref('') // 便携版下载的新 exe 路径
const checking = ref(false)
const dialogOpen = ref(false)
const downloading = ref(false)
const downloadReady = ref(false)
const downloadedBytes = ref(0)
const contentLength = ref(0)
const downloadError = ref('')

const hasUpdate = computed(() => update.value !== null || portableInfo.value !== null)
const isPortable = computed(() => portableInfo.value !== null)
const updateVersion = computed(() => update.value?.version ?? portableInfo.value?.latest_version ?? '')
const progressPercent = computed(() => {
  if (!contentLength.value) return 0
  return Math.min(100, Math.round((downloadedBytes.value / contentLength.value) * 100))
})

function isNotPortable(e: unknown) {
  return String(e).includes('NOT_PORTABLE')
}

// 双路径检查：便携版优先，Err("NOT_PORTABLE") 回退插件路径。成功完成返回 true。
async function checkForUpdate(silent: boolean): Promise<boolean> {
  try {
    const info = await invoke<PortableUpdateInfo>('portable_check_update')
    portableInfo.value = info.has_update ? info : null
    update.value = null
    return true
  } catch (e) {
    if (isNotPortable(e)) {
      // 安装版，走插件 check()
      try {
        update.value = await check()
        portableInfo.value = null
        return true
      } catch (e2) {
        console.error('[updater] 检查更新失败', e2)
        if (!silent) toast('检查更新失败', 'error')
        return false
      }
    }
    console.error('[updater] 便携版检查更新失败', e)
    if (!silent) toast('检查更新失败', 'error')
    return false
  }
}

async function silentCheck() {
  await checkForUpdate(true)
}

async function manualCheck() {
  if (checking.value || downloading.value) return
  checking.value = true
  try {
    if (await checkForUpdate(false)) {
      if (hasUpdate.value) {
        dialogOpen.value = true
      } else {
        toast('已是最新版本', 'success')
      }
    }
  } finally {
    checking.value = false
  }
}

function onCheckClick() {
  if (downloading.value) return
  if (hasUpdate.value) {
    // 静默检查已发现更新，直接打开对话框
    dialogOpen.value = true
    return
  }
  manualCheck()
}

function closeDialog() {
  if (downloading.value) return // 下载中不允许关闭，避免进度丢失
  dialogOpen.value = false
}

async function startDownload() {
  if (downloading.value) return
  if (isPortable.value) {
    await portableDownload()
  } else if (update.value) {
    await pluginDownload()
  }
}

// 安装版：插件 downloadAndInstall + relaunch
async function pluginDownload() {
  const u = update.value
  if (!u) return
  downloading.value = true
  downloadReady.value = false
  downloadError.value = ''
  downloadedBytes.value = 0
  contentLength.value = 0
  try {
    await u.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        contentLength.value = event.data.contentLength ?? 0
      } else if (event.event === 'Progress') {
        downloadedBytes.value += event.data.chunkLength
      } else if (event.event === 'Finished') {
        downloadReady.value = true
      }
    })
    downloadReady.value = true
    // Windows 上安装器会自动运行并退出，relaunch 兜底其余平台
    setTimeout(relaunchNow, 1500)
  } catch (e) {
    console.error('[updater] 下载更新失败', e)
    downloadError.value = String(e)
  } finally {
    downloading.value = false
  }
}

// 便携版：自制下载命令 + 进度事件 + 替换脚本安装
async function portableDownload() {
  const info = portableInfo.value
  if (!info) return
  downloading.value = true
  downloadReady.value = false
  downloadError.value = ''
  downloadedBytes.value = 0
  contentLength.value = 0
  let unlisten: (() => void) | null = null
  try {
    unlisten = await listen<{ downloaded: number; total: number }>('portable-update-progress', (e) => {
      contentLength.value = e.payload.total
      downloadedBytes.value = e.payload.downloaded
    })
    installedPath.value = await invoke<string>('portable_download_update', {
      downloadUrl: info.download_url,
      signatureUrl: info.signature_url,
    })
    downloadReady.value = true
    // 替换脚本随后接管并退出应用，无需 relaunch
    setTimeout(installPortable, 1500)
  } catch (e) {
    console.error('[updater] 便携版下载更新失败', e)
    downloadError.value = String(e)
  } finally {
    unlisten?.()
    downloading.value = false
  }
}

async function installPortable() {
  try {
    await invoke('portable_install', { newExePath: installedPath.value })
  } catch (e) {
    console.error('[updater] 便携版安装失败', e)
    downloadReady.value = false
    downloadError.value = String(e)
  }
}

async function relaunchNow() {
  if (isPortable.value) {
    await installPortable()
    return
  }
  try {
    await relaunch()
  } catch (e) {
    // Windows 安装器接管退出时 relaunch 抛错属预期
    console.error('[updater] 重启失败', e)
  }
}

const collapsed = ref(false)
let manualCollapse: boolean | null = null

const pageTitle = computed(() => {
  const map: Record<string, string> = {
    '/': '智能控制台', '/import': '收集票据', '/match': '核对匹配', '/export': '打包导出', '/debug': '调试工具', '/settings': '报销标准设置',
  }
  return map[route.path] || '发票报销助手'
})

const navItems = computed(() => [
  { to: '/', label: '控制台', icon: Home, badge: 0 },
  { to: '/import', label: '收集票据', icon: Upload, badge: invoiceStore.parseErrors.length },
  { to: '/match', label: '核对匹配', icon: Link2, badge: matchStore.unmatchedInvoices.length + matchStore.unmatchedPayments.length },
  { to: '/export', label: '打包导出', icon: Download, badge: matchStore.trips.length },
])

function navLinkClass(to: string) {
  const active = route.path === to || (to !== '/' && route.path.startsWith(to))
  return active
    ? 'bg-gradient-to-r from-primary-600 to-accent-500 text-white shadow-glow-sm'
    : 'text-slate-600 hover:bg-white/80 hover:text-primary-700 hover:shadow-card'
}

function handleResize() {
  const narrow = window.innerWidth < 1024
  if (manualCollapse === null) collapsed.value = narrow
}

function toggleCollapse() {
  collapsed.value = !collapsed.value
  manualCollapse = collapsed.value
}

onMounted(() => {
  handleResize()
  useEventListener(window, 'resize', handleResize)
  initOcrStatus()
  silentCheck()
})
</script>

<style scoped>
.modal-enter-active, .modal-leave-active { transition: opacity 0.25s ease; }
.modal-enter-from, .modal-leave-to { opacity: 0; }
</style>
