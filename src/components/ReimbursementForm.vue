<template>
  <div class="bg-white rounded-lg border p-5 shadow-sm space-y-4">
    <h3 class="font-medium text-lg">报销信息</h3>

    <div class="grid grid-cols-2 gap-4">
      <div>
        <label class="block text-sm text-gray-600 mb-1">姓名</label>
        <input v-model="form.name" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入姓名" />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">部职别</label>
        <input v-model="form.department" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入部职别" />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">到达地点</label>
        <input v-model="form.destination" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入到达地点" />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">同行人数</label>
        <input v-model.number="form.companions" type="number" min="0" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">出差开始日期</label>
        <input v-model="form.travelStart" type="date" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">出差结束日期</label>
        <input v-model="form.travelEnd" type="date" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">住宿级别</label>
        <select v-model="form.hotelLevel" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500">
          <option value="其他人员">其他人员</option>
          <option value="师级">师级</option>
          <option value="军级">军级</option>
          <option value="战区级以上">战区级以上</option>
        </select>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue'

const form = reactive({
  name: '',
  department: '',
  destination: '',
  travelStart: '',
  travelEnd: '',
  companions: 0,
  hotelLevel: '其他人员',
})

interface FormState {
  name: string
  department: string
  destination: string
  travelStart: string
  travelEnd: string
  companions: number
  hotelLevel: string
}

const emit = defineEmits<{
  (e: 'update', form: FormState): void
}>()

watch(form, (val) => emit('update', val), { deep: true })
</script>
