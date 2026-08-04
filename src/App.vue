<template>
  <div class="h-screen flex bg-gray-50">
    <!-- 侧栏 -->
    <aside class="flex flex-col bg-white border-r border-gray-200 shrink-0 transition-all duration-200"
           :class="collapsed ? 'w-14' : 'w-48'">
      <div class="flex items-center gap-2 h-14 px-3 border-b border-gray-100">
        <div class="w-8 h-8 rounded-lg bg-primary-600 text-white flex items-center justify-center shrink-0">
          <AppIcon name="clipboard" :size="18" />
        </div>
        <div v-if="!collapsed" class="min-w-0">
          <p class="text-sm font-semibold text-gray-800 leading-tight truncate">发票报销助手</p>
          <p class="text-[11px] text-gray-400 leading-tight">v{{ version }}</p>
        </div>
      </div>
      <nav class="flex-1 py-2 space-y-0.5">
        <router-link v-for="item in navItems" :key="item.to" :to="item.to" :title="item.label"
                     class="flex items-center gap-3 mx-2 px-3 py-2 rounded-lg text-sm transition-colors"
                     :class="navLinkClass(item.to)">
          <AppIcon :name="item.icon" :size="18" class="shrink-0" />
          <span v-if="!collapsed" class="truncate">{{ item.label }}</span>
        </router-link>
      </nav>
      <div class="p-2 border-t border-gray-100">
        <router-link to="/debug" class="flex items-center gap-3 mx-2 px-3 py-2 rounded-lg text-sm hover:bg-gray-100"
                     :class="navLinkClass('/debug')">
          <AppIcon name="debug" :size="18" class="shrink-0" />
          <span v-if="!collapsed">调试工具</span>
        </router-link>
      </div>
    </aside>

    <!-- 主区 -->
    <div class="flex-1 flex flex-col min-w-0">
      <header class="flex items-center justify-between h-14 px-6 bg-white/70 backdrop-blur border-b border-gray-200 shrink-0">
        <div class="flex items-center gap-3 min-w-0">
          <button @click="collapsed = !collapsed" class="text-gray-400 hover:text-gray-600" :aria-label="collapsed ? '展开侧栏' : '收起侧栏'" :title="collapsed ? '展开侧栏' : '收起侧栏'">
            <AppIcon name="swap" :size="18" />
          </button>
          <h1 class="text-sm font-semibold text-gray-800 truncate">{{ pageTitle }}</h1>
        </div>
        <div class="flex items-center gap-4">
          <span class="flex items-center gap-1.5 text-xs text-gray-500">
            <span class="w-2 h-2 rounded-full" :class="ocrOnline ? 'bg-emerald-500' : 'bg-red-500'" />
            OCR {{ ocrOnline ? '在线' : '离线' }}
          </span>
        </div>
      </header>
      <main class="flex-1 overflow-auto">
        <div class="max-w-5xl mx-auto px-6 py-5">
          <AppStepper class="mb-5" />
          <router-view />
        </div>
      </main>
    </div>

    <AppToast />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import AppIcon from './components/ui/AppIcon.vue'
import AppStepper from './components/ui/AppStepper.vue'
import AppToast from './components/ui/AppToast.vue'
import { useOcrStatus, initOcrStatus } from './composables/ocr'
import pkg from '../package.json'

const version = pkg.version
const route = useRoute()
const collapsed = ref(false)
const { ocrOnline } = useOcrStatus()

const pageTitle = computed(() => {
  const map: Record<string, string> = {
    '/': '首页', '/import': '导入', '/match': '匹配', '/export': '导出', '/debug': '调试工具',
  }
  return map[route.path] || '发票报销助手'
})

const navItems = [
  { to: '/', label: '首页', icon: 'home' as const },
  { to: '/import', label: '导入', icon: 'upload' as const },
  { to: '/match', label: '匹配', icon: 'link' as const },
  { to: '/export', label: '导出', icon: 'download' as const },
]

function navLinkClass(to: string) {
  const active = route.path === to || (to !== '/' && route.path.startsWith(to))
  return active
    ? 'bg-primary-50 text-primary-700 font-medium'
    : 'text-gray-600 hover:bg-gray-100'
}

onMounted(() => {
  collapsed.value = window.innerWidth < 1024
  initOcrStatus()
})
</script>
