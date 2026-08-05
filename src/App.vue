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

    <AppToast />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useEventListener } from '@vueuse/core'
import {
  PanelLeftClose, PanelLeftOpen, Home, Upload, Link2, Download, Bug, Settings, Receipt,
} from 'lucide-vue-next'
import AppStepper from './components/ui/AppStepper.vue'
import AppToast from './components/ui/AppToast.vue'
import { useOcrStatus, initOcrStatus } from './composables/ocr'
import { useInvoiceStore } from './stores/invoice'
import { useMatchStore } from './stores/match'
import pkg from '../package.json'

const version = pkg.version
const route = useRoute()
const invoiceStore = useInvoiceStore()
const matchStore = useMatchStore()
const { ocrOnline } = useOcrStatus()

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
})
</script>
