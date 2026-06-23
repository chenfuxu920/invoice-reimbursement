<template>
  <div class="bg-white border rounded-lg p-5">
    <h3 class="font-medium text-gray-700 mb-3">标注模式</h3>
    <p class="text-gray-400 text-sm">（待实现）</p>
  </div>
</template>

<script setup lang="ts">
import type { InvoiceTemplate } from '../types'
defineProps<{ template: InvoiceTemplate }>()
defineEmits<{ 'update-field': [fieldName: string, pattern: string] }>()
</script>
