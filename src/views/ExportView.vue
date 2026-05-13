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

      <!-- 报销信息表单 -->
      <ReimbursementForm @update="handleFormUpdate" class="mb-6" />

      <!-- 预览 -->
      <div class="flex gap-3 mb-6">
        <button @click="previewForm"
                class="px-4 py-2 rounded bg-blue-500 text-white hover:bg-blue-600 transition-colors">
          预览报销单
        </button>
      </div>

      <!-- 报销单预览（直接渲染表格） -->
      <div v-if="matchStore.reimbursementHtml" class="border rounded-lg overflow-hidden mb-6">
        <div class="bg-gray-100 px-4 py-2 text-sm text-gray-600">
          <span>报销单预览</span>
        </div>
        <div class="p-4 overflow-auto" style="min-height: 600px;">
          <div v-html="matchStore.reimbursementHtml"></div>
        </div>
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
</script>