<template>
  <div class="bg-white border rounded-lg p-5 mt-6">
    <h3 class="font-medium text-gray-700 mb-3">实时测试</h3>
    <p class="text-gray-400 text-sm">（待实现）</p>
  </div>
</template>

<script setup lang="ts">
import type { InvoiceTemplate } from '../types'
defineProps<{ template: InvoiceTemplate }>()
</script>
