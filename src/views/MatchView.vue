<template>
  <div class="max-w-4xl mx-auto">
    <div class="flex justify-between items-center mb-6">
      <h2 class="text-2xl font-bold">匹配结果</h2>
      <button @click="runAutoMatch"
              :disabled="invoiceStore.invoices.length === 0 || paymentStore.payments.length === 0 || matchStore.loading"
              class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50">
        {{ matchStore.loading ? '匹配中...' : '自动匹配' }}
      </button>
    </div>

    <!-- 匹配结果 -->
    <div v-if="matchStore.matches.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3">已匹配 ({{ matchStore.matches.length }})</h3>
      <div class="grid gap-3">
        <MatchCard v-for="m in matchStore.matches" :key="m.invoice_id" :match="m" @adjust="handleAdjust" />
      </div>
    </div>

    <!-- 未匹配发票 -->
    <div v-if="matchStore.unmatchedInvoices.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3 text-orange-600">未匹配发票 ({{ matchStore.unmatchedInvoices.length }})</h3>
      <div class="grid gap-2">
        <div v-for="inv in matchStore.unmatchedInvoices" :key="inv.id"
             class="bg-orange-50 border border-orange-200 rounded p-3 flex justify-between items-center">
          <div>
            <span class="font-medium">{{ inv.invoice_number || '无编号' }}</span>
            <span class="text-gray-500 ml-2">¥{{ inv.amount.toFixed(2) }}</span>
          </div>
          <button @click="startManualMatch(inv)" class="text-sm text-blue-500 hover:text-blue-700">手动匹配</button>
        </div>
      </div>
    </div>

    <!-- 未匹配支付 -->
    <div v-if="matchStore.unmatchedPayments.length" class="mb-8">
      <h3 class="text-lg font-medium mb-3 text-gray-500">未匹配支付 ({{ matchStore.unmatchedPayments.length }})</h3>
      <div class="text-sm text-gray-400">
        <div v-for="p in matchStore.unmatchedPayments" :key="p.id" class="py-1">
          {{ p.merchant_name }} · ¥{{ p.amount.toFixed(2) }} · {{ p.transaction_time }}
        </div>
      </div>
    </div>

    <!-- 空状态 -->
    <div v-if="!matchStore.matches.length && !matchStore.unmatchedInvoices.length" class="text-center py-12 text-gray-400">
      请先在导入页面添加发票和账单，然后点击自动匹配
    </div>

    <!-- 报销单生成区域（匹配完成后显示） -->
    <div v-if="matchStore.matches.length" class="mt-8 border-t pt-6">
      <h3 class="text-lg font-medium mb-4">生成报销单</h3>

      <!-- 报销信息表单 -->
      <div class="bg-white rounded-lg border p-5 shadow-sm mb-4 space-y-4">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="block text-sm text-gray-600 mb-1">姓名</label>
            <input v-model="formInfo.name" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入姓名" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">部职别</label>
            <input v-model="formInfo.department" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入部职别" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">到达地点</label>
            <input v-model="formInfo.destination" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入到达地点" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">同行人数</label>
            <input v-model.number="formInfo.companions" type="number" min="0" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">出差开始日期</label>
            <input v-model="formInfo.travelStart" type="date" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">出差结束日期</label>
            <input v-model="formInfo.travelEnd" type="date" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" />
          </div>
          <div>
            <label class="block text-sm text-gray-600 mb-1">住宿级别</label>
            <select v-model="formInfo.hotelLevel" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500">
              <option value="其他人员">其他人员</option>
              <option value="师级">师级</option>
              <option value="军级">军级</option>
              <option value="战区级以上">战区级以上</option>
            </select>
          </div>
        </div>

        <div class="flex gap-3">
          <button @click="previewForm" :disabled="!formInfo.name || !formInfo.department"
                  class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50 transition-colors">
            预览报销单
          </button>
          <button @click="downloadForm" :disabled="!formInfo.name || !formInfo.department"
                  class="px-4 py-2 rounded bg-green-500 text-white hover:bg-green-600 disabled:opacity-50 transition-colors">
            下载报销单 HTML
          </button>
        </div>
      </div>

      <!-- 报销单预览 -->
      <div v-if="matchStore.reimbursementHtml" class="border rounded-lg overflow-hidden">
        <div class="bg-gray-100 px-4 py-2 text-sm text-gray-600 flex justify-between items-center">
          <span>报销单预览</span>
          <button @click="showPreview = !showPreview" class="text-blue-500 hover:text-blue-700">
            {{ showPreview ? '收起' : '展开' }}
          </button>
        </div>
        <div v-if="showPreview" class="p-4">
          <iframe :srcdoc="matchStore.reimbursementHtml" class="w-full border-0" style="min-height: 600px;"></iframe>
        </div>
      </div>
    </div>

    <!-- 手动匹配对话框 -->
    <MatchAdjustDialog
      :visible="showAdjustDialog"
      :invoice="adjustingInvoice"
      :available-payments="matchStore.unmatchedPayments"
      @close="showAdjustDialog = false"
      @confirm="handleManualMatch"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useInvoiceStore } from '../stores/invoice'
import { usePaymentStore } from '../stores/payment'
import { useMatchStore } from '../stores/match'
import MatchCard from '../components/MatchCard.vue'
import MatchAdjustDialog from '../components/MatchAdjustDialog.vue'
import type { Invoice, MatchResult } from '../types'

const invoiceStore = useInvoiceStore()
const paymentStore = usePaymentStore()
const matchStore = useMatchStore()

const showAdjustDialog = ref(false)
const adjustingInvoice = ref<Invoice | null>(null)
const showPreview = ref(false)

const formInfo = reactive({
  name: '',
  department: '',
  destination: '',
  travelStart: '',
  travelEnd: '',
  companions: 0,
  hotelLevel: '其他人员',
})

async function runAutoMatch() {
  await matchStore.autoMatch(invoiceStore.invoices, paymentStore.payments)
}

function handleAdjust(match: MatchResult) {
  adjustingInvoice.value = match.invoice
  showAdjustDialog.value = true
}

function startManualMatch(invoice: Invoice) {
  adjustingInvoice.value = invoice
  showAdjustDialog.value = true
}

async function handleManualMatch(invoice: Invoice, paymentIds: string[]) {
  const payments = matchStore.unmatchedPayments.filter(p => paymentIds.includes(p.id))
  await matchStore.manualMatch(invoice, payments)
  showAdjustDialog.value = false
}

async function previewForm() {
  try {
    await matchStore.renderReimbursementHtml(formInfo)
    showPreview.value = true
  } catch (e) {
    console.error('预览失败:', e)
    alert('预览失败: ' + e)
  }
}

async function downloadForm() {
  try {
    const path = await matchStore.saveReimbursementHtml(formInfo)
    alert('报销单已保存: ' + path)
  } catch (e) {
    console.error('保存失败:', e)
    alert('保存失败: ' + e)
  }
}
</script>
