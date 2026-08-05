<template>
  <div class="rounded-2xl border border-slate-200/70 bg-gradient-to-b from-white to-slate-50/60 shadow-card p-5 space-y-4">
    <h3 class="font-display text-base font-bold text-slate-800 flex items-center gap-2">
      <ClipboardList :size="16" class="text-primary-600" />
      报销信息
    </h3>

    <div class="grid grid-cols-2 gap-4">
      <div>
        <label class="block text-sm font-medium text-slate-600 mb-1.5">到达城市</label>
        <input v-model="form.destination" class="input" placeholder="请输入到达城市" />
      </div>
      <div>
        <label class="block text-sm font-medium text-slate-600 mb-1.5">出差开始日期</label>
        <input v-model="form.travelStart" type="date" class="input" />
      </div>
      <div>
        <label class="block text-sm font-medium text-slate-600 mb-1.5">出差结束日期</label>
        <input v-model="form.travelEnd" type="date" class="input" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, watch, nextTick } from 'vue'
import { ClipboardList } from 'lucide-vue-next'

interface FormState {
  destination: string
  travelStart: string
  travelEnd: string
  // 级别选择已移除，保留字段作为报表住宿行标签（固定「其他人员」，不影响金额）
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
