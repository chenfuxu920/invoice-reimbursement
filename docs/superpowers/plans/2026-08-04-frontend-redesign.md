# 前端界面重设计（财务工作台）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将发票报销助手前端从朴素灰底白卡重设计为「财务工作台」视觉（侧栏导航 + 流程步进器 + 统一语义状态色 + Toast 替换 alert()），功能与数据逻辑不变。

**架构：** Tailwind 4 `@theme` 设计令牌 → 新建 `src/components/ui/` 基础组件（Icon/Button/Badge/Empty/Stepper/Toast）→ 重写 App 壳层（侧栏+顶栏+步进器）→ 逐个视图与业务组件换肤 → 替换 alert()。

**技术栈：** Vue 3.4、Tailwind CSS 4、Pinia、vue-router、Tauri 2。无新依赖。

**设计规格：** `docs/superpowers/specs/2026-08-04-frontend-redesign-design.md`

---

## 文件结构

| 文件 | 职责 |
|---|---|
| `src/style.css` | @theme 设计令牌（主色/字体/圆角），全局基础样式 |
| `src/components/ui/AppIcon.vue` | 统一 stroke SVG 图标组件（替换全部 emoji） |
| `src/components/ui/AppButton.vue` | 按钮四态（primary/secondary/ghost/danger）+ loading |
| `src/components/ui/AppBadge.vue` | 状态徽章（语义色圆点+文字） |
| `src/components/ui/AppEmpty.vue` | 空状态（图标+文案+操作按钮） |
| `src/components/ui/AppStepper.vue` | 3 步流程指示条（导入→匹配→导出） |
| `src/composables/toast.ts` | toast 状态与 API（替换 alert） |
| `src/components/ui/AppToast.vue` | toast 渲染（右上角，3 秒消失） |
| `src/utils/category.ts` | 分类样式改为主色语义 + 图标名（去掉 emoji） |
| `src/App.vue` | 壳层：侧栏 + 顶栏 + 主区 |
| `src/views/*.vue` | 5 个视图换肤 |
| `src/components/*.vue` | 业务组件换肤（卡片/拖拽区/表格/弹窗等） |
| `src/__tests__/ui.test.ts` | 可测逻辑单元测试（toast、stepper、category） |

---

### 任务 1：设计令牌与全局样式

**文件：**
- 修改：`src/style.css`

- [ ] **步骤 1：写入设计令牌**

```css
@import "tailwindcss";

@theme {
  --font-sans: system-ui, -apple-system, "Segoe UI", "Microsoft YaHei", "PingFang SC", sans-serif;
  --color-primary-50: #eef2ff;
  --color-primary-100: #e0e7ff;
  --color-primary-200: #c7d2fe;
  --color-primary-500: #6366f1;
  --color-primary-600: #4f46e5;
  --color-primary-700: #4338ca;
  --radius-card: 10px;
  --radius-btn: 8px;
}

html, body, #app {
  height: 100%;
}

body {
  @apply bg-gray-50 text-gray-800 antialiased;
  background-image: linear-gradient(to bottom, #f9fafb, #f3f4f6);
}

/* 金额数字等宽对齐 */
.tabular-nums { font-variant-numeric: tabular-nums; }
```

- [ ] **步骤 2：验证构建**

运行：`npm run build:check`
预期：PASS（vite build 成功）

- [ ] **步骤 3：Commit**

```bash
git add src/style.css
git commit -m "style: 引入财务工作台设计令牌（主色/字体/圆角）"
```

---

### 任务 2：图标组件 AppIcon

**文件：**
- 创建：`src/components/ui/AppIcon.vue`

- [ ] **步骤 1：编写组件（含图标集）**

```vue
<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path v-for="d in paths" :key="d" :d="d" />
    <circle v-for="c in circles" :key="c" :cx="c[0]" :cy="c[1]" :r="c[2]" />
  </svg>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type IconName =
  | 'home' | 'upload' | 'link' | 'download' | 'debug'
  | 'eye' | 'doc' | 'image' | 'table' | 'x'
  | 'chevron-down' | 'plus' | 'spinner' | 'check' | 'alert'
  | 'train' | 'plane' | 'shield' | 'swap' | 'car'
  | 'hotel' | 'meal' | 'toll' | 'clipboard'

const props = withDefaults(defineProps<{ name: IconName; size?: number }>(), { size: 16 })

const ICONS: Record<IconName, { paths: string[]; circles?: [number, number, number][] }> = {
  home: { paths: ['M3 10.5 12 3l9 7.5', 'M5 9.5V21h14V9.5', 'M9 21v-6h6v6'] },
  upload: { paths: ['M12 16V4', 'm6 10 6-6 6 6', 'M4 20h16'] },
  link: { paths: ['M9 15l6-6', 'M11 6.5 13 4.5a4 4 0 0 1 5.7 5.7l-2 2', 'M13 17.5 11 19.5a4 4 0 0 1-5.7-5.7l2-2'] },
  download: { paths: ['M12 4v12', 'm6 10 6 6 6-6', 'M4 20h16'] },
  debug: { paths: ['M4 8l4-4M20 8l-4-4', 'M12 3v4', 'M3 12h4M17 12h4', 'M12 20v-6', 'M8 17l-3 3M16 17l3 3'] },
  eye: { paths: ['M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z', 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z'] },
  doc: { paths: ['M6 2h8l4 4v16H6z', 'M14 2v4h4', 'M9 12h6M9 16h6'] },
  image: { paths: ['M4 4h16v16H4z', 'M4 15l4-4 3 3 5-5 4 4', 'M8.5 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z'] },
  table: { paths: ['M4 4h16v16H4z', 'M4 9h16M4 14h16M10 4v16'] },
  x: { paths: ['M6 6l12 12M18 6 6 18'] },
  'chevron-down': { paths: ['m6 9 6 6 6-6'] },
  plus: { paths: ['M12 5v14M5 12h14'] },
  spinner: { paths: ['M12 3a9 9 0 1 0 9 9'] },
  check: { paths: ['m5 12 5 5L20 7'] },
  alert: { paths: ['M12 3 2 20h20L12 3Z', 'M12 10v4', 'M12 17.5v.5'] },
  train: { paths: ['M6 4h12a2 2 0 0 1 2 2v9H4V6a2 2 0 0 1 2-2Z', 'M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3', 'M8 19l-2 2M16 19l2 2', 'M8 10h8', 'M7 14h.01M17 14h.01'] },
  plane: { paths: ['M3 10.5 12 3l9 7.5', 'M12 3v9', 'M7 10.5 12 21l5-10.5'] },
  shield: { paths: ['M12 3 4 6v6c0 5 3.5 8.5 8 9 4.5-.5 8-4 8-9V6l-8-3Z', 'M9 12l2 2 4-4'] },
  swap: { paths: ['M4 7h13M17 4l3 3-3 3', 'M20 17H7M7 14l-3 3 3 3'] },
  car: { paths: ['M5 11 7 6h10l2 5', 'M4 11h16v6H4z', 'M7 17v2M17 17v2', 'M7 13h.01M17 13h.01'] },
  hotel: { paths: ['M4 21V5a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v16', 'M14 9h5a1 1 0 0 1 1 1v11', 'M2 21h20', 'M7 8h.01M10 8h.01M7 12h.01M10 12h.01'] },
  meal: { paths: ['M7 3v7a2 2 0 0 0 4 0V3', 'M9 3v18', 'M16 3c-1.5 2-1.5 6 0 8v10', 'M16 11c1.5-1 1.5-4 0-8'] },
  toll: { paths: ['M5 5h14l-4 7 4 7H5l4-7-4-7Z'] },
  clipboard: { paths: ['M9 4h6v3H9z', 'M6 4h3l6 0h3a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z'] },
}

const paths = computed(() => ICONS[props.name]?.paths ?? [])
const circles = computed(() => ICONS[props.name]?.circles ?? [])
</script>
```

- [ ] **步骤 2：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 3：Commit**

```bash
git add src/components/ui/AppIcon.vue
git commit -m "feat(ui): AppIcon 统一图标组件（替换 emoji）"
```

---

### 任务 3：UI 基础组件（Button / Badge / Empty）

**文件：**
- 创建：`src/components/ui/AppButton.vue`
- 创建：`src/components/ui/AppBadge.vue`
- 创建：`src/components/ui/AppEmpty.vue`

- [ ] **步骤 1：编写 AppButton**

```vue
<template>
  <button :disabled="disabled || loading" class="inline-flex items-center justify-center gap-1.5 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          :class="[sizeClass, variantClass]" :title="title">
    <svg v-if="loading" class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
    </svg>
    <slot />
  </button>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md'
  disabled?: boolean
  loading?: boolean
  title?: string
}>(), { variant: 'secondary', size: 'md' })

const sizeClass = computed(() =>
  props.size === 'sm' ? 'px-2.5 py-1 text-xs' : 'px-3.5 py-1.5'
)
const variantClass = computed(() => ({
  primary: 'bg-primary-600 text-white hover:bg-primary-700',
  secondary: 'bg-white text-gray-700 border border-gray-300 hover:bg-gray-50',
  ghost: 'text-gray-600 hover:bg-gray-100',
  danger: 'bg-red-500 text-white hover:bg-red-600',
}[props.variant]))
</script>
```

- [ ] **步骤 2：编写 AppBadge**

```vue
<template>
  <span class="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium"
        :class="toneClass">
    <span class="w-1.5 h-1.5 rounded-full" :class="dotClass" />
    <slot />
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type BadgeTone = 'success' | 'warning' | 'danger' | 'neutral' | 'info'

const props = withDefaults(defineProps<{ tone?: BadgeTone }>(), { tone: 'neutral' })

const toneClass = computed(() => ({
  success: 'bg-emerald-50 text-emerald-700',
  warning: 'bg-amber-50 text-amber-700',
  danger: 'bg-red-50 text-red-700',
  neutral: 'bg-gray-100 text-gray-600',
  info: 'bg-primary-50 text-primary-700',
}[props.tone]))

const dotClass = computed(() => ({
  success: 'bg-emerald-500',
  warning: 'bg-amber-500',
  danger: 'bg-red-500',
  neutral: 'bg-gray-400',
  info: 'bg-primary-500',
}[props.tone]))
</script>
```

- [ ] **步骤 3：编写 AppEmpty**

```vue
<template>
  <div class="flex flex-col items-center justify-center py-14 text-center">
    <div class="w-12 h-12 rounded-full bg-gray-100 flex items-center justify-center text-gray-400 mb-3">
      <AppIcon :name="icon" :size="24" />
    </div>
    <p class="text-sm text-gray-500 mb-4">{{ message }}</p>
    <slot />
  </div>
</template>

<script setup lang="ts">
import AppIcon, { type IconName } from './AppIcon.vue'
withDefaults(defineProps<{ icon?: IconName; message: string }>(), { icon: 'alert' })
</script>
```

- [ ] **步骤 4：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/components/ui/AppButton.vue src/components/ui/AppBadge.vue src/components/ui/AppEmpty.vue
git commit -m "feat(ui): AppButton/AppBadge/AppEmpty 基础组件"
```

---

### 任务 4：Toast 系统（替换 alert）

**文件：**
- 创建：`src/composables/toast.ts`
- 创建：`src/components/ui/AppToast.vue`
- 测试：`src/__tests__/ui.test.ts`

- [ ] **步骤 1：编写失败的测试**

```ts
import { describe, it, expect, vi } from 'vitest'

describe('toast', () => {
  it('push 后 3 秒自动移除', async () => {
    vi.useFakeTimers()
    const { toast, toasts, removeToast } = await import('../composables/toast')
    toast('成功', 'success')
    expect(toasts.value).toHaveLength(1)
    expect(toasts.value[0].message).toBe('成功')
    vi.advanceTimersByTime(3000)
    expect(toasts.value).toHaveLength(0)
    removeToast(0)
    vi.useRealTimers()
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm test -- src/__tests__/ui.test.ts`
预期：FAIL（`toast` 模块不存在，导入报错）

- [ ] **步骤 3：编写 toast composable**

```ts
import { ref } from 'vue'

export interface ToastItem {
  id: number
  type: 'success' | 'error' | 'info'
  message: string
}

const toasts = ref<ToastItem[]>([])
let nextId = 1

export function toast(message: string, type: ToastItem['type'] = 'info') {
  const id = nextId++
  toasts.value.push({ id, type, message })
  setTimeout(() => removeToast(id), 3000)
}

export function removeToast(id: number) {
  toasts.value = toasts.value.filter(t => t.id !== id)
}

export function useToast() {
  return { toasts, toast, removeToast }
}
```

- [ ] **步骤 4：编写 AppToast 渲染组件**

```vue
<template>
  <Teleport to="body">
    <div class="fixed top-4 right-4 z-[60] space-y-2 w-80">
      <TransitionGroup name="toast">
        <div v-for="t in toasts" :key="t.id"
             class="flex items-start gap-2.5 rounded-lg border bg-white p-3 shadow-lg"
             :class="borderClass(t.type)">
          <span class="w-2 h-2 rounded-full mt-1.5 shrink-0" :class="dotClass(t.type)" />
          <p class="text-sm text-gray-700 flex-1 break-words">{{ t.message }}</p>
          <button class="text-gray-400 hover:text-gray-600 shrink-0" @click="removeToast(t.id)">
            <AppIcon name="x" :size="14" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import AppIcon from './AppIcon.vue'
import { useToast } from '../../composables/toast'
import type { ToastItem } from '../../composables/toast'

const { toasts, removeToast } = useToast()

function borderClass(type: ToastItem['type']) {
  return { success: 'border-emerald-200', error: 'border-red-200', info: 'border-primary-200' }[type]
}
function dotClass(type: ToastItem['type']) {
  return { success: 'bg-emerald-500', error: 'bg-red-500', info: 'bg-primary-500' }[type]
}
</script>

<style scoped>
.toast-enter-active, .toast-leave-active { transition: all 0.25s ease; }
.toast-enter-from { opacity: 0; transform: translateX(16px); }
.toast-leave-to { opacity: 0; transform: translateX(16px); }
</style>
```

- [ ] **步骤 5：运行测试验证通过**

运行：`npm test -- src/__tests__/ui.test.ts`
预期：PASS

- [ ] **步骤 6：在 App.vue 挂载 AppToast（先占位，任务 6 再整体重写 App.vue 时保留）**

在 `src/App.vue` 模板 `<main>` 之后加：

```vue
<AppToast />
```

并在 script 中 `import AppToast from './components/ui/AppToast.vue'`

- [ ] **步骤 7：Commit**

```bash
git add src/composables/toast.ts src/components/ui/AppToast.vue src/__tests__/ui.test.ts src/App.vue
git commit -m "feat(ui): Toast 系统（替换 alert 的基础设施）"
```

---

### 任务 5：流程步进器 AppStepper

**文件：**
- 创建：`src/components/ui/AppStepper.vue`

- [ ] **步骤 1：编写组件**

```vue
<template>
  <nav class="flex items-center gap-2 overflow-x-auto py-1">
    <template v-for="(step, i) in steps" :key="step.to">
      <button :disabled="!step.enabled" @click="router.push(step.to)"
              class="flex items-center gap-2 rounded-full px-3 py-1.5 text-sm whitespace-nowrap transition-colors disabled:cursor-not-allowed"
              :class="stepBtnClass(i)">
        <span class="w-5 h-5 rounded-full flex items-center justify-center text-xs font-semibold"
              :class="stepCircleClass(i)">{{ i + 1 }}</span>
        <span :class="stepTextClass(i)">{{ step.label }}</span>
      </button>
      <span v-if="i < steps.length - 1" class="w-5 h-px bg-gray-300 shrink-0" />
    </template>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useInvoiceStore } from '../../stores/invoice'
import { usePaymentStore } from '../../stores/payment'
import { useMatchStore } from '../../stores/match'

const router = useRouter()
const route = useRoute()
const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const steps = computed(() => [
  { to: '/import', label: '导入', enabled: true },
  {
    to: '/match',
    label: '匹配',
    enabled: invoiceStore.invoices.length > 0 && paymentStore.payments.length > 0,
  },
  { to: '/export', label: '导出', enabled: matchStore.matches.length > 0 },
])

function stepBtnClass(i: number) {
  const active = currentIndex.value === i
  return active ? 'bg-primary-50' : steps.value[i].enabled ? 'hover:bg-gray-100' : 'opacity-50'
}
function stepCircleClass(i: number) {
  const active = currentIndex.value === i
  if (active) return 'bg-primary-600 text-white'
  return steps.value[i].enabled ? 'bg-gray-200 text-gray-600' : 'bg-gray-100 text-gray-400'
}
function stepTextClass(i: number) {
  return currentIndex.value === i ? 'text-primary-700 font-medium' : 'text-gray-600'
}

const currentIndex = computed(() => {
  const idx = steps.value.findIndex(s => route.path.startsWith(s.to))
  return idx === -1 ? 0 : idx
})
</script>
```

- [ ] **步骤 2：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 3：Commit**

```bash
git add src/components/ui/AppStepper.vue
git commit -m "feat(ui): AppStepper 流程步进器（按数据状态点亮）"
```

---

### 任务 6：App 壳层（侧栏 + 顶栏 + 步进器）

**文件：**
- 修改：`src/App.vue`

- [ ] **步骤 1：重写 App.vue**

```vue
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
        <router-link v-for="item in navItems" :key="item.to" :to="item.to"
                     class="flex items-center gap-3 mx-2 px-3 py-2 rounded-lg text-sm transition-colors"
                     :class="navLinkClass(item.to)">
          <AppIcon :name="item.icon" :size="18" class="shrink-0" />
          <span v-if="!collapsed" class="truncate">{{ item.label }}</span>
        </router-link>
      </nav>
      <div class="p-2 border-t border-gray-100">
        <router-link to="/debug" class="flex items-center gap-3 mx-2 px-3 py-2 rounded-lg text-sm text-gray-500 hover:bg-gray-100"
                     :class="navLinkClass('/debug')">
          <AppIcon name="debug" :size="18" class="shrink-0" />
          <span v-if="!collapsed">调试工具</span>
        </router-link>
      </div>
    </aside>

    <!-- 主区 -->
    <div class="flex-1 flex flex-col min-w-0">
      <header class="flex items-center justify-between h-14 px-6 bg-white/70 backdrop-blur border-b border-gray-200 shrink-0">
        <button @click="collapsed = !collapsed" class="text-gray-400 hover:text-gray-600" :title="collapsed ? '展开侧栏' : '收起侧栏'">
          <AppIcon name="swap" :size="18" />
        </button>
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
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import AppIcon from './components/ui/AppIcon.vue'
import AppStepper from './components/ui/AppStepper.vue'
import AppToast from './components/ui/AppToast.vue'
import { invoke } from '@tauri-apps/api/core'
import pkg from '../package.json'

const version = pkg.version
const route = useRoute()
const collapsed = ref(false)
const ocrOnline = ref(false)

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

onMounted(async () => {
  try { ocrOnline.value = await invoke('ocr_health') } catch { ocrOnline.value = false }
})
</script>

<style scoped>
@reference "tailwindcss";
</style>
```

注意：`debug` 路由仍保留 `/debug` 路径，不删除路由。

- [ ] **步骤 2：验证构建**

运行：`npm run build:check`
预期：PASS（若 `debug` 路由冲突需先移除 DebugView 顶部的旧结构，见任务 11）

- [ ] **步骤 3：Commit**

```bash
git add src/App.vue
git commit -m "feat(ui): App 壳层重写（侧栏+顶栏+流程步进器）"
```

---

### 任务 7：分类样式去 emoji（utils/category.ts）

**文件：**
- 修改：`src/utils/category.ts`
- 修改：`src/types/invoice.ts`（仅确认 CATEGORY_LABELS 不动）

- [ ] **步骤 1：重写 category.ts**

```ts
import type { InvoiceCategory } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'
import type { IconName } from '../components/ui/AppIcon.vue'

export interface CategoryStyle {
  label: string
  icon: IconName
  badgeClass: string
}

const CATEGORY_STYLES: Record<InvoiceCategory, CategoryStyle> = {
  Train: { label: CATEGORY_LABELS.Train, icon: 'train', badgeClass: 'bg-emerald-50 text-emerald-700' },
  Flight: { label: CATEGORY_LABELS.Flight, icon: 'plane', badgeClass: 'bg-primary-50 text-primary-700' },
  Insurance: { label: CATEGORY_LABELS.Insurance, icon: 'shield', badgeClass: 'bg-cyan-50 text-cyan-700' },
  TicketChange: { label: CATEGORY_LABELS.TicketChange, icon: 'swap', badgeClass: 'bg-amber-50 text-amber-700' },
  CityTransport: { label: CATEGORY_LABELS.CityTransport, icon: 'car', badgeClass: 'bg-purple-50 text-purple-700' },
  Hotel: { label: CATEGORY_LABELS.Hotel, icon: 'hotel', badgeClass: 'bg-yellow-50 text-yellow-700' },
  Meal: { label: CATEGORY_LABELS.Meal, icon: 'meal', badgeClass: 'bg-rose-50 text-rose-700' },
  Toll: { label: CATEGORY_LABELS.Toll, icon: 'toll', badgeClass: 'bg-indigo-50 text-indigo-700' },
  Other: { label: CATEGORY_LABELS.Other, icon: 'clipboard', badgeClass: 'bg-gray-100 text-gray-700' },
}

export function getCategoryStyle(category: InvoiceCategory): CategoryStyle {
  return CATEGORY_STYLES[category] || CATEGORY_STYLES.Other
}

export function getCategoryLabel(category: InvoiceCategory): string {
  return getCategoryStyle(category).label
}

export function getCategoryIcon(category: InvoiceCategory): IconName {
  return getCategoryStyle(category).icon
}

export function getCategoryBadgeClass(category: InvoiceCategory): string {
  return getCategoryStyle(category).badgeClass
}
```

- [ ] **步骤 2：检查所有 `getCategoryIcon` 调用点（它们现在返回 IconName 而非 emoji）**

运行：`rg -n "getCategoryIcon" src/`
预期：找到 `InvoiceCard.vue`（第 7 行用法）。将其改为在模板中渲染 AppIcon：

```vue
<AppIcon :name="getCategoryIcon(invoice.category)" :size="12" />
```

- [ ] **步骤 3：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 4：Commit**

```bash
git add src/utils/category.ts src/components/InvoiceCard.vue
git commit -m "refactor(ui): 分类样式去 emoji，改用 AppIcon"
```

---

### 任务 8：首页重设计

**文件：**
- 修改：`src/views/HomeView.vue`

- [ ] **步骤 1：重写模板结构（保留全部 script 逻辑与调用）**

保持 `<script setup>` 中所有 ref/方法/onMounted 不变（ocrOnline、downloadModels、saveConfig、showConfig、downloadProgress、stores），仅重写模板为：

```vue
<template>
  <div class="space-y-6">
    <!-- OCR 引擎状态卡 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <span class="w-2.5 h-2.5 rounded-full" :class="ocrOnline ? 'bg-emerald-500' : 'bg-red-500'" />
          <span class="font-medium text-gray-800">OCR 识别服务</span>
        </div>
        <AppBadge :tone="ocrOnline ? 'success' : 'danger'">{{ ocrOnline ? '在线' : '离线' }}</AppBadge>
      </div>
      <div v-if="!ocrOnline" class="mt-4 pt-4 border-t border-gray-100 space-y-3">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <p class="text-sm font-medium text-gray-700">OCR 模型</p>
            <p class="text-xs text-gray-400">识别扫描件、图片发票（约 20MB）</p>
          </div>
          <AppButton v-if="!downloadingModels" variant="primary" size="sm" @click="downloadModels">下载</AppButton>
          <span v-else class="text-sm text-primary-600">{{ downloadProgress.file }} ({{ downloadProgress.index + 1 }}/{{ downloadProgress.total }})…</span>
        </div>
        <div>
          <button class="text-xs text-gray-400 hover:text-gray-600" @click="showConfig = !showConfig">⚙ 下载地址设置</button>
          <div v-if="showConfig" class="mt-2 flex gap-2">
            <input v-model="modelBaseUrl" class="flex-1 px-2.5 py-1.5 border rounded-lg text-sm font-mono focus:outline-none focus:border-primary-500"
                   placeholder="https://github.com/.../releases/download/ocr-models-v1" />
            <AppButton size="sm" @click="saveConfig">保存</AppButton>
          </div>
        </div>
      </div>
    </div>

    <!-- 数据统计 -->
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-primary-600 tabular-nums">{{ invoiceStore.invoices.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已导入发票</p>
      </div>
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-emerald-600 tabular-nums">{{ paymentStore.payments.length }}</p>
        <p class="text-sm text-gray-500 mt-1">支付记录</p>
      </div>
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-purple-600 tabular-nums">{{ matchStore.matches.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已匹配</p>
      </div>
      <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 text-center">
        <p class="text-3xl font-bold text-gray-600 tabular-nums">{{ matchStore.trips.length }}</p>
        <p class="text-sm text-gray-500 mt-1">已分趟</p>
      </div>
    </div>

    <!-- 流程引导 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5">
      <h3 class="font-medium text-gray-800 mb-3">下一步</h3>
      <p class="text-sm text-gray-500 mb-4">{{ nextStepHint }}</p>
      <div class="flex gap-3 flex-wrap">
        <AppButton v-if="!hasInvoices" variant="primary" @click="$router.push('/import')">导入发票与账单</AppButton>
        <AppButton v-else-if="!hasMatches" variant="primary" @click="$router.push('/match')">开始匹配</AppButton>
        <AppButton v-else variant="primary" @click="$router.push('/export')">前往导出</AppButton>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5">
      <h3 class="font-medium text-gray-800 mb-4">快捷操作</h3>
      <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
        <router-link to="/import" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-primary-300 hover:bg-primary-50 transition-colors">
          <AppIcon name="upload" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">导入发票</span>
        </router-link>
        <router-link to="/import" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-emerald-300 hover:bg-emerald-50 transition-colors">
          <AppIcon name="table" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">导入账单</span>
        </router-link>
        <router-link to="/match" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-purple-300 hover:bg-purple-50 transition-colors">
          <AppIcon name="link" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">开始匹配</span>
        </router-link>
        <router-link to="/export" class="flex flex-col items-center gap-2 p-4 rounded-lg border border-gray-200 hover:border-amber-300 hover:bg-amber-50 transition-colors">
          <AppIcon name="download" :size="22" class="text-gray-500" />
          <span class="text-sm font-medium text-gray-700">导出报销表</span>
        </router-link>
      </div>
    </div>
  </div>
</template>
```

在 script 中补充（追加到现有代码）：

```ts
import AppButton from '../components/ui/AppButton.vue'
import AppBadge from '../components/ui/AppBadge.vue'
import AppIcon from '../components/ui/AppIcon.vue'

const hasInvoices = computed(() => invoiceStore.invoices.length > 0)
const hasMatches = computed(() => matchStore.matches.length > 0)
const nextStepHint = computed(() => {
  if (!hasInvoices.value) return '先导入发票与微信/支付宝账单，再进行自动匹配。'
  if (!hasMatches.value) return '发票与账单已就绪，点击开始自动匹配。'
  if (matchStore.trips.length === 0) return '匹配完成，前往导出页确认分趟并生成报销表。'
  return '全部就绪，可随时前往导出页生成报销材料。'
})
```

注意：新增 `computed` 需要从 vue 导入（现有 `import { ref, onMounted } from 'vue'` 改为 `import { ref, computed, onMounted } from 'vue'`）。

- [ ] **步骤 2：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 3：Commit**

```bash
git add src/views/HomeView.vue
git commit -m "feat(ui): 首页重设计（状态卡/统计/流程引导/快捷操作）"
```

---

### 任务 9：导入页重设计

**文件：**
- 修改：`src/views/ImportView.vue`
- 修改：`src/components/InvoiceDropZone.vue`
- 修改：`src/components/BillImporter.vue`
- 修改：`src/components/InvoiceCard.vue`
- 修改：`src/components/PaymentTable.vue`

- [ ] **步骤 1：重写 InvoiceDropZone（emoji 加载态 → 图标）**

将 loading 分支改为：

```vue
<div v-if="loading" class="flex items-center justify-center gap-2 text-primary-600">
  <svg class="animate-spin h-5 w-5" viewBox="0 0 24 24" fill="none">
    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
  </svg>
  识别中...
</div>
```

拖拽区容器 class 从 `hover:border-blue-400 hover:bg-blue-50` 改为 `hover:border-primary-400 hover:bg-primary-50`，拖拽态 `border-primary-500 bg-primary-50`。

- [ ] **步骤 2：重写 BillImporter（微信/支付宝选择用语义色）**

- 微信选中：`bg-emerald-600 text-white border-emerald-600`；未选中 `bg-white text-gray-600 border-gray-300 hover:border-emerald-400`
- 支付宝选中：`bg-primary-600 text-white border-primary-600`；未选中 `bg-white text-gray-600 border-gray-300 hover:border-primary-400`
- 加载态同样替换 emoji spinner 为 SVG spinner（同任务 9 步骤 1）
- 拖拽区 hover 色统一为 primary

- [ ] **步骤 3：重写 InvoiceCard**

模板改为（保持全部 props/emits/展开逻辑不变）：

```vue
<template>
  <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-4 cursor-pointer hover:shadow-md transition-shadow"
       @click="expanded = !expanded">
    <div class="flex justify-between items-start gap-3">
      <div class="flex-1 min-w-0">
        <AppBadge :tone="badgeTone(category)">{{ getCategoryLabel(invoice.category) }}</AppBadge>
        <div class="mt-2 cursor-pointer" @click.stop="$emit('view-detail', invoice)">
          <p class="text-lg font-bold tabular-nums">¥{{ invoice.amount.toFixed(2) }}</p>
          <p class="text-sm text-gray-500 hover:text-primary-600 truncate">{{ invoice.seller_name || '未知销售方' }}</p>
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <span class="text-gray-400 transition-transform text-sm" :class="{ 'rotate-180': expanded }">▾</span>
        <button class="text-gray-400 hover:text-red-500" title="删除" @click.stop="$emit('remove', invoice.id)">
          <AppIcon name="x" :size="16" />
        </button>
      </div>
    </div>
    <div class="mt-2 text-xs text-gray-400 flex gap-4">
      <span>发票号: {{ invoice.invoice_number || '无' }}</span>
      <span>日期: {{ invoice.date }}</span>
    </div>
    <div v-if="expanded" class="mt-3 pt-3 border-t border-gray-100 space-y-2 text-sm">
      <!-- 保持展开摘要结构，行程行从 bg-blue-50 改为 bg-primary-50，⚠ 保留 -->
    </div>
  </div>
</template>
```

script 中 `getCategoryIcon` 不再直接输出 emoji——用 `getCategoryLabel` + AppBadge（badgeTone 映射：Train→success, Flight→info, TicketChange→warning, 其余→neutral；可在组件内写一个轻量映射函数，或直接用 `getCategoryBadgeClass` 保留原有色系徽章。**推荐直接用 `getCategoryBadgeClass`**，改动最小）：

实际使用：

```vue
<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium"
      :class="getCategoryBadgeClass(invoice.category)">
  <AppIcon :name="getCategoryIcon(invoice.category)" :size="12" />
  {{ getCategoryLabel(invoice.category) }}
</span>
```

- [ ] **步骤 4：PaymentTable 统一表格样式**

阅读 `src/components/PaymentTable.vue` 后，将容器边框改为 `border-gray-200`、表头 `bg-gray-50 text-gray-500`、金额列加 `tabular-nums`、行 hover `hover:bg-gray-50`。保持 props/emits 不变。

- [ ] **步骤 5：重写 ImportView 模板外层结构**

- 页头按钮区：`清空全部` 用 AppButton variant="danger"（或 secondary）+ `全局导入` 用 AppButton variant="primary"（图标 upload）
- 发票区标题行 + 「＋手动添加空发票」→ AppButton variant="primary" size="sm" + plus 图标
- 错误区：`border-red-200 bg-red-50` → `border-red-200 bg-red-50 rounded-[10px]`，错误条目按钮改用 AppButton size="sm"
- 全部 `alert()` 调用替换为 `toast()`（导入完成/失败/去重提示、清空等）：`toast('全局导入完成…', 'success')`、`toast('已跳过 N 张重复发票…', 'info')`、`toast('导入失败: ' + e, 'error')`
- 在模板顶部加 `<AppToast />` 不需要（App.vue 已全局挂载）

- [ ] **步骤 6：验证构建 + 测试**

运行：`npm run build:check` 与 `npm test`
预期：均 PASS

- [ ] **步骤 7：Commit**

```bash
git add src/views/ImportView.vue src/components/InvoiceDropZone.vue src/components/BillImporter.vue src/components/InvoiceCard.vue src/components/PaymentTable.vue
git commit -m "feat(ui): 导入页重设计 + alert 替换 toast"
```

---

### 任务 10：匹配页重设计

**文件：**
- 修改：`src/views/MatchView.vue`
- 修改：`src/components/MatchCard.vue`

- [ ] **步骤 1：重写 MatchCard 样式**

- 顶部徽章：`matchTypeClass` 保留（已是语义色）；confidenceClass 保留
- 「调整」按钮：`text-sm text-primary-600 hover:text-primary-800`
- 发票/支付子区块：`bg-gray-50 rounded p-2` 保持，hover 微调 `hover:bg-gray-100`
- 移除按钮 `✕` → AppIcon name="x"
- 微信/支付宝来源徽章 `bg-green-100 text-green-700` → 保留（语义已对）

- [ ] **步骤 2：重写 MatchView 结构**

- 「自动匹配」按钮 → AppButton variant="primary"，disabled 逻辑不变
- 未匹配发票区块：标题 `text-orange-600` → `text-amber-600`，条目背景 `bg-orange-50 border-orange-200` → `bg-amber-50 border-amber-200 hover:bg-amber-100`
- 未匹配支付折叠标题 `▶` → AppIcon name="chevron-down"（展开旋转）
- 空状态 → AppEmpty 组件：`<AppEmpty icon="link" message="请先在导入页面添加发票和账单，然后点击自动匹配" />`
- `alert('自动匹配失败: ' + e)` → `toast(..., 'error')`
- 统计区域如已存在保留；若匹配页顶部无统计卡，按规格加三卡（已匹配/未匹配发票/未匹配支付）——阅读现有代码确认后决定（现有 MatchView 没有顶部统计卡，**新增**，复用任务 8 的统计卡样式）

- [ ] **步骤 3：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 4：Commit**

```bash
git add src/views/MatchView.vue src/components/MatchCard.vue
git commit -m "feat(ui): 匹配页重设计（统计卡/语义色/空状态）"
```

---

### 任务 11：导出页 + 调试页重设计

**文件：**
- 修改：`src/views/ExportView.vue`
- 修改：`src/views/DebugView.vue`
- 修改：`src/components/TripCard.vue`
- 修改：`src/components/ExportButton.vue`
- 修改：`src/components/ReimbursementForm.vue`

- [ ] **步骤 1：ExportButton 去 emoji + 图标化**

- 四个操作按钮（报销单 HTML/对照 PDF/报销单 Excel/信息对照单）的 emoji（📄🖼️📊📋）替换为 AppIcon（doc/image/table/clipboard），label 模式与 icon 模式分支保留
- `alert()` → `toast()`（成功/失败，含批量导出完成提示）
- LoadingOverlay 保留

- [ ] **步骤 2：TripCard 重设计**

- 顶部「出差 N」徽章：`bg-blue-100 text-blue-700` → AppBadge tone="info"
- 发票明细折叠区：`border rounded` → `border border-gray-200 rounded-lg`，展开内容行保留
- 预览按钮 emoji（👁/🙈）→ AppIcon eye
- 移除发票 select 样式统一
- `alert('预览失败: ' + e)` → `toast(..., 'error')`

- [ ] **步骤 3：ReimbursementForm 样式**

- 输入框统一：`w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-100`
- 卡片容器与全局卡片一致（rounded-[10px]）

- [ ] **步骤 4：ExportView 结构**

- 顶部摘要三卡沿用统计卡样式（已匹配/未匹配发票/未匹配支付）
- 分趟工具栏/一键导出/待调整区：容器统一 `rounded-[10px] border border-gray-200 shadow-sm`
- 待调整区橙色系 → amber（同任务 10）
- 所有按钮换 AppButton
- `alert()` → `toast()`（重匹配失败/成功等）
- 空状态 → AppEmpty

- [ ] **步骤 5：DebugView 去旧顶栏**

- 阅读 `src/views/DebugView.vue`，移除页面内部重复的页面标题区（如有），统一交由 App 壳层顶栏；只保留调试功能主体。不改后端逻辑。

- [ ] **步骤 6：验证构建 + 测试**

运行：`npm run build:check` 与 `npm test`
预期：均 PASS

- [ ] **步骤 7：Commit**

```bash
git add src/views/ExportView.vue src/views/DebugView.vue src/components/TripCard.vue src/components/ExportButton.vue src/components/ReimbursementForm.vue
git commit -m "feat(ui): 导出页/调试页重设计 + 按钮图标化"
```

---

### 任务 12：弹窗组件换肤

**文件：**
- 修改：`src/components/InvoiceDetailModal.vue`
- 修改：`src/components/ManualInvoiceEntryModal.vue`
- 修改：`src/components/BlankInvoiceEntryModal.vue`
- 修改：`src/components/MatchAdjustDialog.vue`
- 修改：`src/components/PaymentDetailModal.vue`

- [ ] **步骤 1：统一 Modal 外壳样式（逐个文件）**

每个弹窗组件按以下模式统一（保持各自 props/emits/逻辑不变）：

```vue
<!-- 遮罩 -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
  <div class="bg-white rounded-[10px] shadow-2xl w-full max-w-lg max-h-[85vh] flex flex-col">
    <!-- 标题栏 -->
    <div class="flex items-center justify-between px-5 py-4 border-b border-gray-100">
      <h2 class="text-base font-semibold text-gray-800">标题</h2>
      <button class="text-gray-400 hover:text-gray-600" @click="close"><AppIcon name="x" :size="16" /></button>
    </div>
    <!-- 内容区（原表单/表格内容，输入框按 ReimbursementForm 统一样式） -->
    <div class="flex-1 overflow-auto px-5 py-4">...</div>
    <!-- 页脚按钮 -->
    <div class="flex justify-end gap-2 px-5 py-3 border-t border-gray-100">
      <AppButton @click="close">取消</AppButton>
      <AppButton variant="primary" @click="save">保存</AppButton>
    </div>
  </div>
</div>
```

- [ ] **步骤 2：验证构建**

运行：`npm run build:check`
预期：PASS

- [ ] **步骤 3：Commit**

```bash
git add src/components/InvoiceDetailModal.vue src/components/ManualInvoiceEntryModal.vue src/components/BlankInvoiceEntryModal.vue src/components/MatchAdjustDialog.vue src/components/PaymentDetailModal.vue
git commit -m "feat(ui): 弹窗组件统一换肤"
```

---

### 任务 13：残余 alert 清理 + 全局验证

**文件：**
- 修改：`src/**/*.vue`（如有遗漏的 alert）

- [ ] **步骤 1：查找残余 alert**

运行：`rg -n "alert\(" src/`
预期：无输出（若仍有，逐个替换为 `toast(msg, type)` 并引入 `useToast`）

- [ ] **步骤 2：全量构建与测试**

运行：`npm run build:check`
预期：PASS

运行：`npm test`
预期：全部通过

- [ ] **步骤 3：手动验证（浏览器）**

运行：`npm run dev`
手动检查：
1. 侧栏折叠/展开；窄窗口（约 900px、700px）下统计卡列数变化、步进器横向滚动
2. 首页流程引导随数据状态变化
3. 导入/匹配/导出/调试 5 页正常渲染，无 emoji 残留
4. 触发一次 Toast（如导入重复文件）确认右上角弹出且 3 秒消失
5. 弹窗打开/关闭正常

- [ ] **步骤 4：Commit**

```bash
git add src/
git commit -m "feat(ui): 完整财务工作台重设计"
```

---

## 自检

**1. 规格覆盖度：**
- 布局（侧栏/顶栏/步进器）→ 任务 6、5
- 视觉语言（令牌/图标/密度）→ 任务 1、2、3
- 组件规范（按钮/徽章/空状态/Toast/弹窗）→ 任务 3、4、12
- 各页面（首页/导入/匹配/导出/调试）→ 任务 8、9、10、11
- 窄窗口适配 → 任务 8（grid 响应式）、6（侧栏折叠）、5（步进器 overflow-x）
- alert() 替换 → 任务 4、9、10、11、13
- 验证 → 任务 13 步骤 2-3

**2. 占位符扫描：** 无 TODO/待定；所有新组件均有完整代码；现有组件修改给出具体 class 与结构。

**3. 类型一致性：**
- `AppIcon` 的 `IconName` 导出（`export type IconName`）在 `category.ts` 通过 `import type { IconName } from '../components/ui/AppIcon.vue'` 引用，需确认 AppIcon 的 `<script setup>` 中类型是 `export type`（任务 2 已含）。
- `toast`/`useToast`/`removeToast` 签名在 composable 与测试、AppToast、各视图一致。
- `AppButton` props：`variant/size/disabled/loading/title` 各调用点一致。
- `AppStepper` 依赖 `useInvoiceStore/usePaymentStore/useMatchStore` 的 `invoices/payments/matches` 字段，与现有 store 一致（store 字段确认：`invoiceStore.invoices`、`paymentStore.payments`、`matchStore.matches` 均已在视图中使用）。
- `matchStore.trips` 在 HomeView 统计卡使用，现有 ExportView 已用 `matchStore.trips`。
