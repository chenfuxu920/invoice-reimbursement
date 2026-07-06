<template>
  <div
    class="border-2 border-dashed rounded-lg p-8 text-center cursor-pointer
           hover:border-blue-400 hover:bg-blue-50 transition-colors"
    :class="isDragging ? 'border-blue-500 bg-blue-50' : 'border-gray-300'"
    @dragover.prevent="isDragging = true"
    @dragleave="isDragging = false"
    @click="openFilePicker"
  >
    <div v-if="loading" class="text-blue-500">
      <span class="animate-spin inline-block mr-2">⏳</span> 识别中...
    </div>
    <template v-else>
      <p class="text-gray-500">
        {{ isDragging ? '松开以上传' : '拖拽发票文件到此处，或点击选择' }}
      </p>
      <p class="text-sm text-gray-400 mt-2">支持 PDF / 图片 / 多文件</p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'

defineProps<{ loading?: boolean }>()
const emit = defineEmits<{ (e: 'files-selected', paths: string[]): void }>()

const isDragging = ref(false)
let unlisten: (() => void) | null = null

onMounted(async () => {
  unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
      isDragging.value = false
      emit('files-selected', event.payload.paths)
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

async function openFilePicker() {
  const selected = await open({
    multiple: true,
    filters: [{
      name: '发票文件',
      extensions: ['pdf', 'jpg', 'jpeg', 'png']
    }]
  })
  if (selected) {
    emit('files-selected', selected)
  }
}
</script>
