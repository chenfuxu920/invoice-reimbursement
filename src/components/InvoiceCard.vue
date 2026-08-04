<template>
  <div class="bg-white rounded-lg border p-4 shadow-sm cursor-pointer" @click="expanded = !expanded">
    <div class="flex justify-between items-start">
      <div class="flex-1 min-w-0">
        <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium"
              :class="getCategoryBadgeClass(invoice.category)">
          <AppIcon :name="getCategoryIcon(invoice.category)" :size="12" />
          {{ getCategoryLabel(invoice.category) }}
        </span>
        <!-- 标题/金额区：可点击打开详情弹窗（stop 防止触发整卡展开） -->
        <div class="mt-2 cursor-pointer"
             @click.stop="$emit('view-detail', invoice)">
          <p class="text-lg font-bold">¥{{ invoice.amount.toFixed(2) }}</p>
          <p class="text-sm text-gray-500 hover:text-blue-600">{{ invoice.seller_name || '未知销售方' }}</p>
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <span class="text-gray-400 transition-transform text-sm"
              :class="{ 'rotate-180': expanded }"
              :title="expanded ? '收起' : '展开'">▾</span>
        <button @click.stop="$emit('remove', invoice.id)" class="text-gray-400 hover:text-red-500" title="删除">✕</button>
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
             class="bg-blue-50 rounded px-2 py-1 text-xs text-gray-600 flex items-center gap-1">
          <span v-if="it.incomplete_fields?.length"
                class="text-orange-500 cursor-help"
                :title="'缺失: ' + it.incomplete_fields.map(f => fieldLabel(f)).join(', ')">⚠</span>
          <span class="flex-1">{{ it.date_time }} | {{ it.provider }} | {{ it.pickup }} → {{ it.dropoff }} | ¥{{ it.amount.toFixed(2) }}</span>
        </div>
      </div>
      <p v-else class="text-xs text-gray-400">无行程明细</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { Invoice } from '../types'
import AppIcon from './ui/AppIcon.vue'
import { getCategoryLabel, getCategoryBadgeClass, getCategoryIcon } from '../utils/category'

const props = defineProps<{ invoice: Invoice }>()
defineEmits<{
  (e: 'remove', id: string): void
  (e: 'view-detail', invoice: Invoice): void
}>()

const expanded = ref(false)

const sourceFileName = computed(() => {
  const p = props.invoice.source.path
  if (!p) return '手动添加'
  const parts = p.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || p
})

function fieldLabel(f: string): string {
  const map: Record<string, string> = {
    date_time: '时间',
    provider: '服务商',
    pickup: '起点',
    dropoff: '终点',
    amount: '金额',
  }
  return map[f] || f
}
</script>
