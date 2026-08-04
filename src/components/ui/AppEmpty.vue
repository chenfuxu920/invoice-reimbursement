<template>
  <div class="flex flex-col items-center justify-center py-14 text-center">
    <div class="w-12 h-12 rounded-full bg-gray-100 flex items-center justify-center text-gray-400 mb-3">
      <AppIcon :name="icon" :size="24" />
    </div>
    <p class="text-sm text-gray-500 mb-4">{{ message }}</p>
    <slot />
  </div>
</template>

<script setup lang="ts">
import AppIcon, { type IconName } from './AppIcon.vue'
withDefaults(defineProps<{ icon?: IconName; message: string }>(), { icon: 'alert' })
</script>
