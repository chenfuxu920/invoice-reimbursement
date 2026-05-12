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
      <ReimbursementForm @update="formInfo = $event" class="mb-6" />

      <!-- 导出按钮 -->
      <ExportButton
        :match-results="matchStore.matches"
        :unmatched-invoice-ids="matchStore.unmatchedInvoices.map(i => i.id)"
        :unmatched-payment-ids="matchStore.unmatchedPayments.map(p => p.id)"
        :form-info="formInfo"
        :disabled="!formInfo.name || !formInfo.department"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useMatchStore } from '../stores/match'
import ReimbursementForm from '../components/ReimbursementForm.vue'
import ExportButton from '../components/ExportButton.vue'

const matchStore = useMatchStore()

const formInfo = ref({
  name: '',
  department: '',
  destination: '',
  travelStart: '',
  travelEnd: '',
  companions: 0,
  hotelLevel: '其他人员',
})
</script>
