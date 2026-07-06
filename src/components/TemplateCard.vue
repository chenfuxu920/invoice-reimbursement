<template>
  <div class="border rounded-lg p-4 bg-white flex items-center justify-between hover:shadow-md transition-shadow">
    <div class="flex items-center gap-3">
      <span class="text-2xl">📄</span>
      <div>
        <div class="font-medium text-gray-800">{{ template.name }}</div>
        <div class="text-xs text-gray-400">
          ID: {{ template.template_id }} · 优先级: {{ template.priority }}
          <span v-if="template.source === 'Builtin'" class="ml-2 px-1.5 py-0.5 bg-blue-50 text-blue-600 rounded text-xs">内置</span>
          <span v-else class="ml-2 px-1.5 py-0.5 bg-green-50 text-green-600 rounded text-xs">用户</span>
        </div>
      </div>
    </div>
    <div class="flex items-center gap-2">
      <label class="flex items-center gap-1 text-sm text-gray-600 cursor-pointer">
        <input type="checkbox" :checked="template.enabled" @change="$emit('toggle', template.template_id, ($event.target as HTMLInputElement).checked)" class="rounded" />
        启用
      </label>
      <button @click="$emit('test', template.template_id)"
              class="px-3 py-1 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
        测试
      </button>
      <button v-if="template.source === 'Builtin'" @click="$emit('copy', template.template_id)"
              class="px-3 py-1 rounded bg-blue-50 text-blue-600 hover:bg-blue-100 text-sm transition-colors">
        复制
      </button>
      <button v-else @click="$emit('edit', template.template_id)"
              class="px-3 py-1 rounded bg-blue-50 text-blue-600 hover:bg-blue-100 text-sm transition-colors">
        编辑
      </button>
      <button v-if="template.source === 'User'" @click="$emit('delete', template.template_id)"
              class="px-3 py-1 rounded bg-red-50 text-red-600 hover:bg-red-100 text-sm transition-colors">
        删除
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { TemplateMeta } from '../types'

defineProps<{ template: TemplateMeta }>()
defineEmits<{
  toggle: [id: string, enabled: boolean]
  test: [id: string]
  copy: [id: string]
  edit: [id: string]
  delete: [id: string]
}>()
</script>
