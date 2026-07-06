# 导入界面发票/账单详情查看 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在导入界面（`/import`）支持发票卡片内联展开摘要 + 点击打开详情弹窗（含文件预览），账单行内联展开全部字段，解析失败项独立错误区并支持点击打开手动填写弹窗（左缩略图 + 右表单）。

**架构：** 复用现有 `InvoiceDetailModal`（已含文件预览 + 行程明细）。改造 `InvoiceCard`（整卡展开 + 标题区 emit `view-detail`）、`PaymentTable`（行展开）。新建 `ManualInvoiceEntryModal`（左缩略图 + 右表单，核心字段 + 可展开行程明细录入）。`ImportView` 新增错误区并接入两个弹窗。`invoiceStore` 新增 `parseErrors` 状态及增删方法。

**技术栈：** Vue 3 (script setup + TS) + Pinia + Tailwind CSS + Tauri 2.x invoke + Vitest（store 逻辑 TDD；组件无 `@vue/test-utils`，用 `vue-tsc --noEmit` 类型检查验证）

**规格文档：** `docs/superpowers/specs/2026-06-24-import-detail-view-design.md`

---

## 文件结构

| 文件 | 动作 | 职责 |
|---|---|---|
| `src/types/invoice.ts` | 修改 | 新增 `ParseError` 接口 |
| `src/stores/invoice.ts` | 修改 | 新增 `parseErrors` 状态 + `addParseErrors`/`removeParseError`/`clearParseErrors`/`addManualInvoice` 方法 |
| `src/__tests__/invoice-store-errors.test.ts` | 创建 | store 错误状态方法的 TDD 测试 |
| `src/components/InvoiceCard.vue` | 修改 | 整卡可展开内联摘要；标题/金额区 emit `view-detail` |
| `src/components/PaymentTable.vue` | 修改 | 行可展开显示全部字段 |
| `src/components/ManualInvoiceEntryModal.vue` | 创建 | 左缩略图 + 右表单（核心字段 + 可展开行程明细） |
| `src/views/ImportView.vue` | 修改 | 新增错误区；接入 `InvoiceDetailModal` + `ManualInvoiceEntryModal`；导入后写入 `parseErrors` |

---

## 任务 1：新增 ParseError 类型

**文件：**
- 修改：`src/types/invoice.ts`（在文件末尾追加）

- [ ] **步骤 1：追加 ParseError 接口**

在 `src/types/invoice.ts` 末尾（第 47 行 `CATEGORY_LABELS` 之后）追加：

```typescript

/// 解析失败的文件条目，用于导入界面错误区展示与手动填写入口
export interface ParseError {
  id: string
  filePath: string
  fileName: string
  message: string
}
```

- [ ] **步骤 2：类型检查通过**

运行：`npx vue-tsc --noEmit`
预期：无新增错误（既有 Rust LSP 报错与本任务无关，TS 侧应通过）

- [ ] **步骤 3：Commit**

```bash
git add src/types/invoice.ts
git commit -m "feat(types): 新增 ParseError 类型用于导入错误区"
```

---

## 任务 2：invoiceStore 错误状态方法（TDD）

**文件：**
- 创建：`src/__tests__/invoice-store-errors.test.ts`
- 修改：`src/stores/invoice.ts`

- [ ] **步骤 1：编写失败的测试**

创建 `src/__tests__/invoice-store-errors.test.ts`：

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { createPinia, setActivePinia } from 'pinia'
import { useInvoiceStore } from '../stores/invoice'
import type { Invoice, ParseError } from '../types'

function makeInvoice(id: string): Invoice {
  return {
    id,
    invoice_number: 'INV' + id,
    amount: 100.0,
    seller_name: '测试销售方',
    item_name: '测试项目',
    date: '2025-01-15',
    category: 'Hotel',
    source: { type: 'Photo', path: `/img/${id}.jpg` },
    itineraries: [],
  }
}

function makeError(id: string): ParseError {
  return { id, filePath: `/err/${id}.pdf`, fileName: `${id}.pdf`, message: '解析失败' }
}

describe('invoiceStore 解析错误状态', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('addParseErrors 批量追加错误到 parseErrors', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1'), makeError('e2')])
    expect(store.parseErrors).toHaveLength(2)
    expect(store.parseErrors.map(e => e.id)).toEqual(['e1', 'e2'])
  })

  it('addParseErrors 在已有错误基础上追加（不覆盖）', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1')])
    store.addParseErrors([makeError('e2')])
    expect(store.parseErrors).toHaveLength(2)
  })

  it('removeParseError 按 id 移除单条错误', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1'), makeError('e2'), makeError('e3')])
    store.removeParseError('e2')
    expect(store.parseErrors).toHaveLength(2)
    expect(store.parseErrors.map(e => e.id)).toEqual(['e1', 'e3'])
  })

  it('clearParseErrors 清空全部错误', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1'), makeError('e2')])
    store.clearParseErrors()
    expect(store.parseErrors).toHaveLength(0)
  })

  it('addManualInvoice 将发票加入 invoices 列表', () => {
    const store = useInvoiceStore()
    const inv = makeInvoice('m1')
    store.addManualInvoice(inv)
    expect(store.invoices).toHaveLength(1)
    expect(store.invoices[0].id).toBe('m1')
  })

  it('addManualInvoice 后 removeParseError 配合使用：手动填写保存后错误被移除', () => {
    const store = useInvoiceStore()
    store.addParseErrors([makeError('e1')])
    store.addManualInvoice(makeInvoice('m1'))
    store.removeParseError('e1')
    expect(store.invoices).toHaveLength(1)
    expect(store.parseErrors).toHaveLength(0)
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npx vitest run src/__tests__/invoice-store-errors.test.ts`
预期：FAIL，报错 `store.addParseErrors is not a function`（方法尚未实现）

- [ ] **步骤 3：实现 store 方法**

修改 `src/stores/invoice.ts`。在 `import` 行追加 `ParseError` 类型导入：

```typescript
import type { Invoice, InvoiceCategory, ParseError } from '../types'
```

在 `const loading = ref(false)` 之后新增 `parseErrors` 状态：

```typescript
  const parseErrors = ref<ParseError[]>([])
```

在 `clearInvoices` 函数之后、`return` 之前新增四个方法：

```typescript
  /// 批量追加解析错误（导入后调用，在已有基础上追加）
  function addParseErrors(errors: ParseError[]) {
    parseErrors.value.push(...errors)
  }

  /// 移除单条解析错误（手动填写保存后或重试成功后调用）
  function removeParseError(id: string) {
    parseErrors.value = parseErrors.value.filter(e => e.id !== id)
  }

  /// 清空全部解析错误
  function clearParseErrors() {
    parseErrors.value = []
  }

  /// 手动填写的发票加入列表（不做去重，用户已确认内容）
  function addManualInvoice(invoice: Invoice) {
    invoices.value.push(invoice)
  }
```

修改 `clearInvoices` 函数体，同时清空错误：

```typescript
  function clearInvoices() {
    invoices.value = []
    parseErrors.value = []
  }
```

修改 `return` 语句，导出新成员：

```typescript
  return {
    invoices, loading, parseErrors,
    addInvoice, addInvoicesSkipDuplicates, removeInvoice, updateCategory, clearInvoices,
    addParseErrors, removeParseError, clearParseErrors, addManualInvoice,
  }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`npx vitest run src/__tests__/invoice-store-errors.test.ts`
预期：PASS（6 个测试全部通过）

- [ ] **步骤 5：运行全部测试确保无回归**

运行：`npx vitest run`
预期：全部 PASS（含既有 dedup 测试）

- [ ] **步骤 6：Commit**

```bash
git add src/stores/invoice.ts src/__tests__/invoice-store-errors.test.ts
git commit -m "feat(store): invoiceStore 新增 parseErrors 状态及手动填写支持"
```

---

## 任务 3：InvoiceCard 改造（整卡展开 + view-detail）

**文件：**
- 修改：`src/components/InvoiceCard.vue`（整体重写 template + script）

- [ ] **步骤 1：重写 InvoiceCard.vue**

将 `src/components/InvoiceCard.vue` 全文替换为：

```vue
<template>
  <div class="bg-white rounded-lg border p-4 shadow-sm">
    <div class="flex justify-between items-start">
      <div class="flex-1 min-w-0">
        <span class="inline-block px-2 py-0.5 rounded text-xs font-medium"
              :class="getCategoryBadgeClass(invoice.category)">
          {{ getCategoryIcon(invoice.category) }} {{ getCategoryStyle(invoice.category).label }}
        </span>
        <!-- 标题/金额区：可点击打开详情弹窗 -->
        <div class="mt-2 cursor-pointer hover:text-blue-600 transition-colors"
             @click.stop="$emit('view-detail', invoice)">
          <p class="text-lg font-bold">¥{{ invoice.amount.toFixed(2) }}</p>
          <p class="text-sm text-gray-500 hover:text-blue-600">{{ invoice.seller_name || '未知销售方' }}</p>
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <button @click.stop="expanded = !expanded"
                class="text-gray-400 hover:text-gray-600 transition-transform"
                :class="{ 'rotate-180': expanded }"
                :title="expanded ? '收起' : '展开'">
          ▾
        </button>
        <button @click="$emit('remove', invoice.id)" class="text-gray-400 hover:text-red-500" title="删除">✕</button>
      </div>
    </div>
    <div class="mt-2 text-xs text-gray-400 flex gap-4">
      <span>发票号: {{ invoice.invoice_number || '无' }}</span>
      <span>日期: {{ invoice.date }}</span>
    </div>

    <!-- 内联展开摘要 -->
    <div v-if="expanded" class="mt-3 pt-3 border-t space-y-2 text-sm">
      <div class="flex gap-4 text-gray-600">
        <span>商品/服务: {{ invoice.item_name || '无' }}</span>
        <span>来源文件: {{ sourceFileName }}</span>
      </div>
      <div v-if="invoice.itineraries?.length">
        <p class="text-xs text-gray-500 mb-1">行程明细 ({{ invoice.itineraries.length }})</p>
        <div v-for="(it, i) in invoice.itineraries" :key="i"
             class="bg-blue-50 rounded px-2 py-1 text-xs text-gray-600">
          {{ it.date_time }} | {{ it.provider }} | {{ it.pickup }} → {{ it.dropoff }} | ¥{{ it.amount.toFixed(2) }}
        </div>
      </div>
      <p v-else class="text-xs text-gray-400">无行程明细</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { Invoice } from '../types'
import { getCategoryStyle, getCategoryBadgeClass, getCategoryIcon } from '../utils/category'

const props = defineProps<{ invoice: Invoice }>()
defineEmits<{
  (e: 'remove', id: string): void
  (e: 'view-detail', invoice: Invoice): void
}>()

const expanded = ref(false)

const sourceFileName = computed(() => {
  const p = props.invoice.source.path
  const parts = p.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || p
})
</script>
```

- [ ] **步骤 2：类型检查通过**

运行：`npx vue-tsc --noEmit`
预期：无新增 TS 错误

- [ ] **步骤 3：Commit**

```bash
git add src/components/InvoiceCard.vue
git commit -m "feat(InvoiceCard): 整卡可展开内联摘要，标题区点击打开详情弹窗"
```

---

## 任务 4：PaymentTable 改造（行展开全部字段）

**文件：**
- 修改：`src/components/PaymentTable.vue`（整体重写）

- [ ] **步骤 1：重写 PaymentTable.vue**

将 `src/components/PaymentTable.vue` 全文替换为：

```vue
<template>
  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="bg-gray-50 border-b">
          <th class="px-3 py-2 text-left font-medium text-gray-600 w-6"></th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">时间</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">商户</th>
          <th class="px-3 py-2 text-right font-medium text-gray-600">金额</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">支付方式</th>
          <th class="px-3 py-2 text-left font-medium text-gray-600">来源</th>
          <th class="px-3 py-2 text-center font-medium text-gray-600">操作</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="p in payments" :key="p.id">
          <tr class="border-b hover:bg-gray-50 cursor-pointer" @click="toggle(p.id)">
            <td class="px-3 py-2 text-gray-400 transition-transform" :class="{ 'rotate-180': expandedId === p.id }">▾</td>
            <td class="px-3 py-2">{{ p.transaction_time }}</td>
            <td class="px-3 py-2">{{ p.merchant_name }}</td>
            <td class="px-3 py-2 text-right font-medium">
              ¥{{ p.amount.toFixed(2) }}
              <span v-if="p.refund_amount > 0 || p.discount > 0" class="text-xs text-gray-400 font-normal">
                <br v-if="p.refund_amount > 0 && p.discount > 0" />
                <template v-if="p.refund_amount > 0 && p.discount > 0"><span class="text-red-400">退¥{{ p.refund_amount.toFixed(2) }}</span> <span class="text-green-400">优¥{{ p.discount.toFixed(2) }}</span></template>
                <template v-else-if="p.refund_amount > 0"><span class="text-red-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span></template>
                <template v-else-if="p.discount > 0"><span class="text-green-400">优惠 ¥{{ p.discount.toFixed(2) }}</span></template>
              </span>
            </td>
            <td class="px-3 py-2 text-gray-500">{{ p.payment_method || '-' }}</td>
            <td class="px-3 py-2">
              <span :class="p.source === 'Wechat' ? 'text-green-600' : 'text-blue-600'">
                {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
              </span>
            </td>
            <td class="px-3 py-2 text-center">
              <button @click.stop="$emit('remove', p.id)" class="text-gray-400 hover:text-red-500">✕</button>
            </td>
          </tr>
          <tr v-if="expandedId === p.id">
            <td colspan="7" class="px-6 py-3 bg-gray-50">
              <div class="grid grid-cols-2 md:grid-cols-3 gap-3 text-xs">
                <div><span class="text-gray-400">交易单号:</span> <span class="text-gray-700">{{ p.transaction_id || '-' }}</span></div>
                <div><span class="text-gray-400">交易时间:</span> <span class="text-gray-700">{{ p.transaction_time }}</span></div>
                <div><span class="text-gray-400">实付金额:</span> <span class="text-gray-700 font-medium">¥{{ p.amount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">原始金额:</span> <span class="text-gray-700">¥{{ p.original_amount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">退款金额:</span> <span class="text-gray-700">¥{{ p.refund_amount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">优惠金额:</span> <span class="text-gray-700">¥{{ p.discount.toFixed(2) }}</span></div>
                <div><span class="text-gray-400">商户名称:</span> <span class="text-gray-700">{{ p.merchant_name }}</span></div>
                <div><span class="text-gray-400">来源:</span> <span class="text-gray-700">{{ p.source === 'Wechat' ? '微信' : '支付宝' }}</span></div>
                <div><span class="text-gray-400">交易类型:</span> <span class="text-gray-700">{{ p.category || '-' }}</span></div>
                <div><span class="text-gray-400">支付方式:</span> <span class="text-gray-700">{{ p.payment_method || '-' }}</span></div>
              </div>
            </td>
          </tr>
        </template>
      </tbody>
    </table>
    <div v-if="!payments.length" class="text-center py-8 text-gray-400">
      暂无支付记录
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { PaymentRecord } from '../types'

defineProps<{ payments: PaymentRecord[] }>()
defineEmits<{ (e: 'remove', id: string): void }>()

const expandedId = ref<string | null>(null)

function toggle(id: string) {
  expandedId.value = expandedId.value === id ? null : id
}
</script>
```

- [ ] **步骤 2：类型检查通过**

运行：`npx vue-tsc --noEmit`
预期：无新增 TS 错误

- [ ] **步骤 3：Commit**

```bash
git add src/components/PaymentTable.vue
git commit -m "feat(PaymentTable): 行可展开显示全部字段"
```

---

## 任务 5：新建 ManualInvoiceEntryModal

**文件：**
- 创建：`src/components/ManualInvoiceEntryModal.vue`

- [ ] **步骤 1：创建组件**

创建 `src/components/ManualInvoiceEntryModal.vue`：

```vue
<template>
  <div v-if="visible" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" @click.self="$emit('close')">
    <div class="bg-white rounded-lg shadow-xl w-[900px] max-h-[85vh] flex flex-col">
      <div class="flex items-center justify-between p-4 border-b shrink-0">
        <h3 class="font-medium">手动填写发票信息</h3>
        <button @click="$emit('close')" class="text-gray-400 hover:text-gray-600 text-xl leading-none">&times;</button>
      </div>

      <div class="flex flex-1 overflow-hidden">
        <!-- 左侧缩略图 -->
        <div class="w-[380px] border-r overflow-y-auto bg-gray-50 p-3">
          <p class="text-xs text-gray-500 mb-2">{{ fileName }}</p>
          <div v-if="previewImages.length > 0" class="space-y-3">
            <div v-for="(img, i) in previewImages" :key="i" class="border rounded overflow-hidden bg-white">
              <img :src="img" class="w-full h-auto" :alt="`第 ${i + 1} 页`" />
              <p v-if="previewImages.length > 1" class="text-xs text-center text-gray-500 py-1 bg-gray-50 border-t">
                第 {{ i + 1 }} / {{ previewImages.length }} 页
              </p>
            </div>
          </div>
          <div v-else-if="loadingPreview" class="text-center py-8 text-gray-400">
            <div class="inline-block w-5 h-5 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin"></div>
            <p class="mt-2 text-sm">正在渲染预览...</p>
          </div>
          <div v-else-if="loadError" class="text-center py-8 text-red-400 text-sm">
            <p>预览加载失败</p>
            <p class="text-xs text-gray-400 mt-1">{{ fileName }}</p>
          </div>
        </div>

        <!-- 右侧表单 -->
        <div class="flex-1 overflow-y-auto p-4">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs text-gray-500 mb-1">发票号</label>
              <input v-model="form.invoice_number" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="发票号码" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">金额 *</label>
              <input v-model.number="form.amount" type="number" step="0.01" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="0.00" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">销售方</label>
              <input v-model="form.seller_name" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="销售方名称" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">商品/服务</label>
              <input v-model="form.item_name" class="w-full border rounded px-2 py-1.5 text-sm" placeholder="商品或服务名称" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">开票日期</label>
              <input v-model="form.date" type="date" class="w-full border rounded px-2 py-1.5 text-sm" />
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">类别</label>
              <select v-model="form.category" class="w-full border rounded px-2 py-1.5 text-sm">
                <option v-for="cat in categoryOptions" :key="cat.value" :value="cat.value">{{ cat.label }}</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">来源类型</label>
              <select v-model="form.source.type" class="w-full border rounded px-2 py-1.5 text-sm">
                <option value="Photo">拍照/图片</option>
                <option value="Pdf">PDF 文件</option>
                <option value="Link">外部链接</option>
              </select>
            </div>
          </div>

          <!-- 行程明细录入区（可展开） -->
          <div class="mt-4 border-t pt-3">
            <button @click="showItinerary = !showItinerary"
                    class="flex items-center gap-1 text-sm text-gray-600 hover:text-gray-800">
              <span class="transition-transform" :class="{ 'rotate-90': showItinerary }">▸</span>
              行程明细 ({{ form.itineraries.length }})
            </button>
            <div v-if="showItinerary" class="mt-2 space-y-2">
              <div v-for="(it, i) in form.itineraries" :key="i" class="flex gap-2 items-start bg-gray-50 rounded p-2">
                <input v-model="it.date_time" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="时间" />
                <input v-model="it.provider" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="平台" />
                <input v-model="it.pickup" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="起点" />
                <input v-model="it.dropoff" class="flex-1 border rounded px-2 py-1 text-xs" placeholder="终点" />
                <input v-model.number="it.amount" type="number" step="0.01" class="w-20 border rounded px-2 py-1 text-xs" placeholder="金额" />
                <button @click="form.itineraries.splice(i, 1)" class="text-gray-400 hover:text-red-500 text-sm">✕</button>
              </div>
              <button @click="addItinerary" class="text-xs text-blue-600 hover:text-blue-800">+ 添加行程</button>
            </div>
          </div>
        </div>
      </div>

      <div class="p-4 border-t flex justify-end gap-2 shrink-0">
        <button @click="$emit('close')" class="px-4 py-2 rounded border hover:bg-gray-50 text-sm">取消</button>
        <button @click="handleSave" :disabled="!form.amount" class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50 text-sm">保存</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Invoice, InvoiceCategory, InvoiceSource, Itinerary } from '../types'
import { CATEGORY_LABELS } from '../types/invoice'

const props = defineProps<{ visible: boolean; filePath: string; errorId: string }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', invoice: Invoice, errorId: string): void
}>()

const previewImages = ref<string[]>([])
const loadingPreview = ref(false)
const loadError = ref(false)
const showItinerary = ref(false)

const categoryOptions = computed(() =>
  (Object.keys(CATEGORY_LABELS) as InvoiceCategory[]).map(v => ({ value: v, label: CATEGORY_LABELS[v] }))
)

const fileName = computed(() => {
  const parts = props.filePath.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || props.filePath
})

const form = reactive({
  invoice_number: '',
  amount: 0,
  seller_name: '',
  item_name: '',
  date: new Date().toISOString().slice(0, 10),
  category: 'Other' as InvoiceCategory,
  source: { type: 'Pdf' as InvoiceSource['type'], path: props.filePath },
  itineraries: [] as Itinerary[],
})

function addItinerary() {
  form.itineraries.push({ date_time: '', provider: '', pickup: '', dropoff: '', amount: 0 })
}

function handleSave() {
  const invoice: Invoice = {
    id: 'manual-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
    invoice_number: form.invoice_number,
    amount: form.amount,
    seller_name: form.seller_name,
    item_name: form.item_name,
    date: form.date,
    category: form.category,
    source: { type: form.source.type, path: props.filePath },
    itineraries: form.itineraries.filter(it => it.date_time || it.provider || it.pickup || it.dropoff || it.amount),
  }
  emit('save', invoice, props.errorId)
}

watch(() => props.visible, async (v) => {
  if (!v || !props.filePath) return
  previewImages.value = []
  loadingPreview.value = true
  loadError.value = false
  form.source.path = props.filePath
  try {
    const isImage = /\.(jpg|jpeg|png)$/i.test(props.filePath)
    if (isImage) {
      // 图片直接用 convertFileSrc 或 render_pdf_preview 均可；这里复用 render_pdf_preview 统一接口
      const paths: string[] = await invoke('render_pdf_preview', { filePath: props.filePath })
      previewImages.value = paths
    } else {
      const paths: string[] = await invoke('render_pdf_preview', { filePath: props.filePath })
      previewImages.value = paths
    }
  } catch (e) {
    console.error('预览加载失败:', e)
    loadError.value = true
  } finally {
    loadingPreview.value = false
  }
})
</script>
```

- [ ] **步骤 2：类型检查通过**

运行：`npx vue-tsc --noEmit`
预期：无新增 TS 错误

- [ ] **步骤 3：Commit**

```bash
git add src/components/ManualInvoiceEntryModal.vue
git commit -m "feat(ManualInvoiceEntryModal): 左缩略图+右表单手动填写发票，支持行程明细录入"
```

---

## 任务 6：ImportView 改造（错误区 + 接入弹窗）

**文件：**
- 修改：`src/views/ImportView.vue`

- [ ] **步骤 1：改造 template**

在 `src/views/ImportView.vue` 中，将发票卡片区块（第 18-24 行）替换为以下内容（新增 `@view-detail` 绑定 + 错误区 + 两个弹窗）：

```vue
    <div class="mb-8">
      <h3 class="text-lg font-medium mb-3">发票上传</h3>
      <InvoiceDropZone :loading="invoiceStore.loading" @files-selected="handleInvoiceFiles" />
      <div v-if="invoiceStore.invoices.length" class="mt-4 grid gap-3">
        <InvoiceCard v-for="inv in invoiceStore.invoices" :key="inv.id" :invoice="inv"
                     @remove="invoiceStore.removeInvoice" @view-detail="openInvoiceDetail" />
      </div>

      <!-- 解析失败错误区 -->
      <div v-if="invoiceStore.parseErrors.length" class="mt-4 border border-red-200 rounded-lg bg-red-50 p-4">
        <h4 class="text-sm font-medium text-red-700 mb-2">解析失败（{{ invoiceStore.parseErrors.length }}）</h4>
        <div class="space-y-2">
          <div v-for="err in invoiceStore.parseErrors" :key="err.id"
               class="flex items-center justify-between bg-white rounded px-3 py-2 border border-red-100">
            <div class="flex-1 min-w-0">
              <p class="text-sm text-gray-700 truncate">{{ err.fileName }}</p>
              <p class="text-xs text-red-500 truncate">{{ err.message }}</p>
            </div>
            <div class="flex gap-2 shrink-0 ml-2">
              <button @click="openManualEntry(err)" class="text-xs px-2 py-1 rounded bg-blue-500 text-white hover:bg-blue-600">手动填写</button>
              <button @click="retryParseError(err)" class="text-xs px-2 py-1 rounded border hover:bg-gray-50">重试</button>
              <button @click="invoiceStore.removeParseError(err.id)" class="text-xs px-2 py-1 rounded text-gray-400 hover:text-red-500">✕</button>
            </div>
          </div>
        </div>
      </div>
    </div>
```

在模板最末尾（`</div>` 闭合根 div 之前，即原第 31 行 `</div>` 之前）追加两个弹窗：

```vue
    <InvoiceDetailModal :visible="detailVisible" :invoice="selectedInvoice" @close="detailVisible = false" />
    <ManualInvoiceEntryModal :visible="manualVisible" :file-path="manualEntryFile" :error-id="manualEntryErrorId"
                             @close="manualVisible = false" @save="handleManualSave" />
```

- [ ] **步骤 2：改造 script**

在 `<script setup>` 的 import 区（第 40-45 行之后）追加两个组件导入：

```typescript
import InvoiceDetailModal from '../components/InvoiceDetailModal.vue'
import ManualInvoiceEntryModal from '../components/ManualInvoiceEntryModal.vue'
import type { Invoice, ParseError } from '../types'
```

在 `const matchStore = useMatchStore()` 之后（第 49 行之后）新增弹窗状态：

```typescript
const detailVisible = ref(false)
const selectedInvoice = ref<Invoice | null>(null)
const manualVisible = ref(false)
const manualEntryFile = ref('')
const manualEntryErrorId = ref('')
```

在 `handleInvoiceFiles` 函数中，将 PDF 错误处理段（原第 90-92 行 `for (const [name, err] of result.errors)` 循环）替换为写入 store：

```typescript
      // 将解析失败项写入 store 错误区
      const errs: ParseError[] = result.errors.map(([name, msg], i) => ({
        id: `pdf-${Date.now()}-${i}`,
        filePath: name,
        fileName: name.replace(/\\/g, '/').split('/').pop() || name,
        message: msg,
      }))
      invoiceStore.addParseErrors(errs)
```

在 `handleGlobalImport` 函数中，将 `allErrors.push(...result.errors)`（原第 153 行）之后追加写入 store：

```typescript
      const errs: ParseError[] = result.errors.map(([name, msg], i) => ({
        id: `global-${Date.now()}-${i}`,
        filePath: name,
        fileName: name.replace(/\\/g, '/').split('/').pop() || name,
        message: msg,
      }))
      invoiceStore.addParseErrors(errs)
```

在 `handleClearAll` 函数中追加清空错误（`clearInvoices` 已内置清空 `parseErrors`，但显式调用确保语义清晰）：

```typescript
function handleClearAll() {
  invoiceStore.clearInvoices()
  paymentStore.clearPayments()
  matchStore.clearMatches()
}
```

（注：`clearInvoices` 已在任务 2 中改为同时清空 `parseErrors`，无需额外调用）

在文件末尾（`handleClearAll` 之后）追加弹窗相关函数：

```typescript
function openInvoiceDetail(invoice: Invoice) {
  selectedInvoice.value = invoice
  detailVisible.value = true
}

function openManualEntry(err: ParseError) {
  manualEntryFile.value = err.filePath
  manualEntryErrorId.value = err.id
  manualVisible.value = true
}

function handleManualSave(invoice: Invoice, errorId: string) {
  invoiceStore.addManualInvoice(invoice)
  invoiceStore.removeParseError(errorId)
  manualVisible.value = false
}

async function retryParseError(err: ParseError) {
  const isImage = /\.(jpg|jpeg|png)$/i.test(err.filePath)
  try {
    if (isImage) {
      const added = await invoiceStore.addInvoice(err.filePath, 'image')
      if (added) invoiceStore.removeParseError(err.id)
    } else {
      const result: { invoices: any[], errors: [string, string][], duplicates: string[] } =
        await invoke('batch_recognize', { filePaths: [err.filePath] })
      if (result.invoices.length > 0) {
        invoiceStore.addInvoicesSkipDuplicates(result.invoices)
        invoiceStore.removeParseError(err.id)
      } else if (result.errors.length === 0 && result.duplicates.length > 0) {
        // 重复也算成功，移除错误
        invoiceStore.removeParseError(err.id)
      }
    }
  } catch (e) {
    console.error('重试解析失败:', e)
    alert('重试失败: ' + e)
  }
}
```

- [ ] **步骤 3：类型检查通过**

运行：`npx vue-tsc --noEmit`
预期：无新增 TS 错误

- [ ] **步骤 4：运行全部测试确保无回归**

运行：`npx vitest run`
预期：全部 PASS

- [ ] **步骤 5：Commit**

```bash
git add src/views/ImportView.vue
git commit -m "feat(ImportView): 新增解析失败错误区，接入发票详情弹窗与手动填写弹窗"
```

---

## 任务 7：整体验证

- [ ] **步骤 1：完整类型检查**

运行：`npx vue-tsc --noEmit`
预期：无错误

- [ ] **步骤 2：完整测试套件**

运行：`npx vitest run`
预期：全部 PASS

- [ ] **步骤 3：生产构建检查**

运行：`npm run build:check`
预期：构建成功

- [ ] **步骤 4：手动验证清单（需启动应用）**

启动：`npm run tauri dev`

逐项验证：
1. 导入含正常发票 PDF 的文件夹 → 发票卡片列表显示，点击卡片展开显示商品/来源/行程摘要，再点收起
2. 点击发票卡片标题/金额区 → 弹出发票详情弹窗，显示文件预览 + 行程明细
3. 导入含解析失败文件的文件夹 → 错误区显示失败文件名 + 原因
4. 点击错误条目"手动填写" → 弹出手动填写弹窗，左侧显示缩略图，右侧表单可填写
5. 手动填写表单保存 → 发票加入列表，错误区对应条目消失
6. 点击错误条目"重试" → 重新解析，成功则移除错误项
7. 导入微信/支付宝账单 → 表格行点击展开显示全部字段（交易单号、原始金额、退款、优惠等）
8. 点击"清空全部" → 发票、账单、错误区全部清空

- [ ] **步骤 5：最终 Commit（如有验证修复）**

```bash
git add -A
git commit -m "test: 导入界面详情查看功能整体验证通过"
```
