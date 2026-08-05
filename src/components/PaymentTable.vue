<template>
  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="bg-slate-50/80 border-b border-slate-200">
          <th class="px-4 py-3 text-left font-medium text-slate-500 w-6"></th>
          <th class="px-3 py-3 text-left font-medium text-slate-500">时间</th>
          <th class="px-3 py-3 text-left font-medium text-slate-500">商户</th>
          <th class="px-3 py-3 text-right font-medium text-slate-500">金额</th>
          <th class="px-3 py-3 text-left font-medium text-slate-500">支付方式</th>
          <th class="px-3 py-3 text-left font-medium text-slate-500">来源</th>
          <th class="px-3 py-3 text-center font-medium text-slate-500">操作</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="p in visiblePayments" :key="p.id">
          <tr class="border-b border-slate-100 hover:bg-primary-50/40 cursor-pointer transition-colors" @click="toggle(p.id)">
            <td class="px-4 py-2.5 text-slate-300">
              <ChevronDown :size="14" class="transition-transform duration-200" :class="{ 'rotate-180': expandedId === p.id }" />
            </td>
            <td class="px-3 py-2.5 text-slate-600">{{ p.transaction_time || '-' }}</td>
            <td class="px-3 py-2.5 font-medium text-slate-700">{{ p.merchant_name || '-' }}</td>
            <td class="px-3 py-2.5 text-right font-semibold tabular-nums text-slate-800">
              ¥{{ p.amount.toFixed(2) }}
              <span v-if="p.refund_amount > 0 || p.discount > 0" class="text-xs text-slate-400 font-normal">
                <template v-if="p.refund_amount > 0 && p.discount > 0"><span class="text-rose-400">退¥{{ p.refund_amount.toFixed(2) }}</span> <span class="text-emerald-400">优¥{{ p.discount.toFixed(2) }}</span></template>
                <template v-else-if="p.refund_amount > 0"><span class="text-rose-400">退款 ¥{{ p.refund_amount.toFixed(2) }}</span></template>
                <template v-else-if="p.discount > 0"><span class="text-emerald-400">优惠 ¥{{ p.discount.toFixed(2) }}</span></template>
              </span>
            </td>
            <td class="px-3 py-2.5 text-slate-500">{{ p.payment_method || '-' }}</td>
            <td class="px-3 py-2.5">
              <span class="chip border !py-0.5 whitespace-nowrap -ml-2.5" :class="p.source === 'Wechat' ? 'bg-emerald-50 text-emerald-700 border-emerald-200/70' : 'bg-primary-50 text-primary-700 border-primary-200/70'">
                {{ p.source === 'Wechat' ? '微信' : '支付宝' }}
              </span>
            </td>
            <td class="px-3 py-2.5 text-center">
              <button @click.stop="handleRemove(p.id)"
                      class="w-7 h-7 rounded-lg inline-flex items-center justify-center text-slate-300 hover:text-rose-500 hover:bg-rose-50 transition-all"
                      :title="'删除'" :aria-label="'删除'">
                <Trash2 :size="14" />
              </button>
            </td>
          </tr>
          <tr v-if="expandedId === p.id">
            <td :colspan="7" class="px-6 py-4 bg-gradient-to-b from-slate-50/80 to-white">
              <div class="grid grid-cols-2 md:grid-cols-3 gap-x-6 gap-y-2.5 text-xs animate-fade-in">
                <div><span class="text-slate-400">交易单号:</span> <span class="text-slate-700 break-all">{{ p.transaction_id || '-' }}</span></div>
                <div><span class="text-slate-400">交易时间:</span> <span class="text-slate-700">{{ p.transaction_time }}</span></div>
                <div><span class="text-slate-400">实付金额:</span> <span class="text-slate-800 font-semibold">¥{{ p.amount.toFixed(2) }}</span></div>
                <div><span class="text-slate-400">原始金额:</span> <span class="text-slate-700">¥{{ p.original_amount.toFixed(2) }}</span></div>
                <div><span class="text-slate-400">退款金额:</span> <span class="text-slate-700">¥{{ p.refund_amount.toFixed(2) }}</span></div>
                <div><span class="text-slate-400">优惠金额:</span> <span class="text-slate-700">¥{{ p.discount.toFixed(2) }}</span></div>
                <div><span class="text-slate-400">商户名称:</span> <span class="text-slate-700">{{ p.merchant_name }}</span></div>
                <div><span class="text-slate-400">来源:</span> <span class="text-slate-700">{{ p.source === 'Wechat' ? '微信' : '支付宝' }}</span></div>
                <div><span class="text-slate-400">交易类型:</span> <span class="text-slate-700">{{ p.category || '-' }}</span></div>
                <div><span class="text-slate-400">支付方式:</span> <span class="text-slate-700">{{ p.payment_method || '-' }}</span></div>
              </div>
            </td>
          </tr>
        </template>
      </tbody>
    </table>

    <div v-if="!payments.length" class="text-center py-10 text-slate-400 text-sm">
      暂无支付记录
    </div>

    <!-- 分页控件 -->
    <div v-if="totalPages > 1" class="flex flex-wrap items-center justify-between gap-3 px-4 py-3 border-t border-slate-100 bg-slate-50/50">
      <span class="text-xs text-slate-400">共 {{ payments.length }} 条 · 第 {{ currentPage }} / {{ totalPages }} 页</span>
      <div class="flex items-center gap-1.5">
        <button :disabled="currentPage <= 1" @click="goPage(currentPage - 1)"
                class="px-2.5 py-1.5 rounded-lg text-xs font-medium border border-slate-200 bg-white text-slate-600 transition-all disabled:opacity-40 disabled:cursor-not-allowed hover:border-primary-300 hover:text-primary-600">
          上一页
        </button>
        <button v-for="p in pageButtons" :key="p"
                :class="p === currentPage
                  ? 'px-2.5 py-1.5 rounded-lg text-xs font-bold bg-gradient-to-r from-primary-600 to-accent-500 text-white shadow-glow-sm'
                  : 'px-2.5 py-1.5 rounded-lg text-xs font-medium border border-slate-200 bg-white text-slate-600 hover:border-primary-300 hover:text-primary-600'"
                @click="goPage(p)">
          {{ p }}
        </button>
        <button :disabled="currentPage >= totalPages" @click="goPage(currentPage + 1)"
                class="px-2.5 py-1.5 rounded-lg text-xs font-medium border border-slate-200 bg-white text-slate-600 transition-all disabled:opacity-40 disabled:cursor-not-allowed hover:border-primary-300 hover:text-primary-600">
          下一页
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { ChevronDown, Trash2 } from 'lucide-vue-next'
import type { PaymentRecord } from '../types'

const props = defineProps<{ payments: PaymentRecord[] }>()
const emit = defineEmits<{ (e: 'remove', id: string): void }>()

const expandedId = ref<string | null>(null)

// ponytail: 上千条支付记录全量渲染表格会拖垮路由切换（卸载巨型 DOM），故分页渲染，
// 每页固定 100 条；数据少时不显示分页控件。
const PAGE_SIZE = 100
const currentPage = ref(1)
const totalPages = computed(() => Math.max(1, Math.ceil(props.payments.length / PAGE_SIZE)))
const visiblePayments = computed(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  return props.payments.slice(start, start + PAGE_SIZE)
})

// 页码按钮：最多显示 7 个，含省略跳转（当前页居中窗口）
const pageButtons = computed<number[]>(() => {
  const total = totalPages.value
  if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1)
  const cur = currentPage.value
  const start = Math.max(1, Math.min(cur - 3, total - 6))
  return Array.from({ length: 7 }, (_, i) => start + i)
})

function goPage(p: number) {
  const clamped = Math.min(Math.max(1, p), totalPages.value)
  if (clamped === currentPage.value) return
  currentPage.value = clamped
  expandedId.value = null
}

function toggle(id: string) {
  expandedId.value = expandedId.value === id ? null : id
}

function handleRemove(id: string) {
  emit('remove', id)
  // 当前页最后一条被删除后回退一页，避免停留在空页
  if (visiblePayments.value.length === 1 && currentPage.value > 1) {
    currentPage.value -= 1
  }
}
</script>
