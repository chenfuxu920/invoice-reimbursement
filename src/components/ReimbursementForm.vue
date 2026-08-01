<template>
  <div class="bg-white rounded-lg border p-5 shadow-sm space-y-4">
    <h3 class="font-medium text-lg">报销信息</h3>

    <div class="grid grid-cols-2 gap-4">
      <div>
        <label class="block text-sm text-gray-600 mb-1">到达城市</label>
        <input v-model="form.destination" class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500" placeholder="请输入到达城市" />
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
import { reactive, watch, nextTick } from 'vue'

interface FormState {
  destination: string
  travelStart: string
  travelEnd: string
  hotelLevel: string
}

const props = defineProps<{
  modelValue?: FormState
}>()

const emit = defineEmits<{
  (e: 'update', form: FormState): void
}>()

const form = reactive<FormState>({
  destination: '',
  travelStart: '',
  travelEnd: '',
  hotelLevel: '其他人员',
})

// 父→子同步（如「从票据提取」按钮批量更新）；flag 防止与下方 form watch 形成回环
// immediate: 挂载即同步初始值（否则预填的 destination/起止日期不显示）
let syncing = false
watch(() => props.modelValue, (val) => {
  if (!val || syncing) return
  syncing = true
  Object.assign(form, val)
  nextTick(() => { syncing = false })
}, { deep: true, immediate: true })

// 子→父同步（用户手动编辑输入框）
watch(form, (val) => {
  if (syncing) return
  emit('update', val)
}, { deep: true })
</script>