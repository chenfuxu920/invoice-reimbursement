<template>
  <div class="max-w-4xl mx-auto">
    <h2 class="text-2xl font-bold mb-6">导出报销表</h2>

    <div v-if="matchStore.matches.length === 0" class="text-center py-12 text-gray-400">
      请先在匹配页面完成发票与账单的匹配
    </div>

    <template v-else>
      <!-- 匹配摘要 -->
      <div class="bg-white rounded-lg border p-4 shadow-sm mb-6">
        <div class="grid grid-cols-3 gap-4 text-center">
          <div>
            <p class="text-2xl font-bold text-blue-600">{{ matchStore.matches.length }}</p>
            <p class="text-sm text-gray-500">已匹配</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-orange-500">{{ matchStore.unmatchedInvoices.length }}</p>
            <p class="text-sm text-gray-500">未匹配发票</p>
          </div>
          <div>
            <p class="text-2xl font-bold text-gray-400">{{ matchStore.unmatchedPayments.length }}</p>
            <p class="text-sm text-gray-500">未匹配支付</p>
          </div>
        </div>
      </div>

      <!-- 从票据提取 -->
      <div class="mb-4">
        <button
          @click="extractTripFromTickets"
          class="px-4 py-2 rounded bg-green-500 text-white hover:bg-green-600 transition-colors text-sm"
        >
          🎫 从票据提取
        </button>
      </div>

      <!-- 报销信息表单 -->
      <ReimbursementForm @update="handleFormUpdate" class="mb-6" />

      <!-- 预览 -->
      <div class="flex gap-3 mb-6">
        <button @click="previewForm"
                class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 transition-colors">
          预览报销单
        </button>
      </div>

      <!-- 报销单预览（iframe 隔离样式，防止全局 CSS 泄漏） -->
      <div v-if="matchStore.reimbursementHtml" class="border rounded-lg overflow-hidden mb-6">
        <div class="bg-gray-100 px-4 py-2 text-sm text-gray-600">
          <span>报销单预览</span>
        </div>
        <iframe
          :srcdoc="matchStore.reimbursementHtml"
          class="w-full"
          style="min-height: 600px; border: none;"
          title="报销单预览"
        />
      </div>

      <!-- 导出按钮 -->
      <ExportButton
        :match-results="matchStore.matches"
        :unmatched-invoice-ids="matchStore.unmatchedInvoices.map(i => i.id)"
        :unmatched-payment-ids="matchStore.unmatchedPayments.map(p => p.id)"
        :form-info="exportFormInfo"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
import { reactive, computed } from 'vue'
import { useMatchStore } from '../stores/match'
import ReimbursementForm from '../components/ReimbursementForm.vue'
import ExportButton from '../components/ExportButton.vue'

const matchStore = useMatchStore()

const formInfo = reactive({
  destination: '',
  travelStart: '',
  travelEnd: '',
  hotelLevel: '其他人员',
})

const exportFormInfo = computed(() => ({
  name: '',
  department: '',
  destination: formInfo.destination,
  travelStart: formInfo.travelStart,
  travelEnd: formInfo.travelEnd,
  companions: 0,
  hotelLevel: formInfo.hotelLevel,
}))

function handleFormUpdate(val: { destination: string; travelStart: string; travelEnd: string; hotelLevel: string }) {
  formInfo.destination = val.destination
  formInfo.travelStart = val.travelStart
  formInfo.travelEnd = val.travelEnd
  formInfo.hotelLevel = val.hotelLevel
}

async function previewForm() {
  try {
    await matchStore.renderReimbursementHtml(exportFormInfo.value)
  } catch (e) {
    console.error('预览失败:', e)
    alert('预览失败: ' + e)
  }
}

function extractTripFromTickets() {
  // 过滤 Train/Flight 类且有到达城市的发票
  const tickets = matchStore.matches
    .filter(m => {
      const inv = m.invoice
      return (inv.category === 'Train' || inv.category === 'Flight') && inv.arrivalCity && inv.date
    })
    .map(m => m.invoice)

  if (tickets.length === 0) {
    alert('未找到可提取的火车票或机票')
    return
  }

  // 按日期排序（字符串比较，格式为 "YYYY-MM-DD" 可直接比较）
  tickets.sort((a, b) => a.date.localeCompare(b.date))

  // 目的地 = 最早一张票的到达城市
  const dest = tickets[0].arrivalCity
  if (!dest) {
    alert('票据数据异常：缺少到达城市')
    return
  }
  formInfo.destination = dest

  // 日期范围 = min/max
  formInfo.travelStart = tickets[0].date
  formInfo.travelEnd = tickets[tickets.length - 1].date
}
</script>