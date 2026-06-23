<template>
  <div class="bg-white border rounded-lg p-5">
    <h3 class="font-medium text-gray-700 mb-4">标注模式</h3>

    <!-- 步骤1：上传 PDF -->
    <div class="mb-4">
      <label class="block text-sm text-gray-600 mb-1">① 上传发票 PDF</label>
      <div class="flex gap-2">
        <button @click="selectPdf" class="px-3 py-1.5 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
          选择文件
        </button>
        <span v-if="pdfPath" class="text-sm text-gray-500 self-center truncate flex-1">📄 {{ pdfPath }}</span>
      </div>
    </div>

    <!-- 步骤2：选择字段类型 -->
    <div class="mb-4">
      <label class="block text-sm text-gray-600 mb-1">② 选择要标注的字段类型</label>
      <div class="flex flex-wrap gap-2">
        <button v-for="ft in fieldTypes" :key="ft.value"
                @click="selectedFieldType = ft.value"
                :class="['px-3 py-1 rounded text-sm transition-colors', selectedFieldType === ft.value ? 'bg-blue-600 text-white' : 'bg-gray-100 text-gray-700 hover:bg-gray-200']">
          {{ ft.label }}
        </button>
      </div>
    </div>

    <!-- 步骤3：OCR 文本拖选 -->
    <div class="mb-4" v-if="ocrText">
      <label class="block text-sm text-gray-600 mb-1">③ 在下方文本中拖选该字段对应的内容</label>
      <div ref="ocrTextRef"
           @mouseup="handleTextSelection"
           class="border rounded p-3 bg-gray-50 text-sm font-mono whitespace-pre-wrap max-h-64 overflow-auto cursor-text select-text">
        {{ ocrText }}
      </div>
    </div>
    <div v-else-if="loadingOcr" class="text-center py-4 text-gray-400 text-sm">OCR 识别中...</div>

    <!-- 步骤4：生成的正则 -->
    <div v-if="generatedRegex !== null" class="mb-4">
      <label class="block text-sm text-gray-600 mb-1">④ 生成的正则（可手动修改）</label>
      <textarea v-model="editableRegex" rows="2"
                class="w-full border rounded px-3 py-2 text-sm font-mono"></textarea>
      <div class="flex gap-2 mt-2">
        <button @click="confirmField" class="px-3 py-1 rounded bg-green-600 text-white hover:bg-green-700 text-sm transition-colors">
          确认此字段
        </button>
        <button @click="generatedRegex = null" class="px-3 py-1 rounded bg-gray-100 text-gray-700 hover:bg-gray-200 text-sm transition-colors">
          取消
        </button>
      </div>
    </div>

    <!-- 标注进度 -->
    <div class="border-t pt-3">
      <div class="text-sm text-gray-600 mb-2">标注进度：</div>
      <div class="flex flex-wrap gap-2">
        <span v-for="ft in fieldTypes" :key="ft.value"
              :class="['px-2 py-0.5 rounded text-xs', isFieldAnnotated(ft.value) ? 'bg-green-50 text-green-600' : 'bg-gray-50 text-gray-400']">
          {{ ft.label }} {{ isFieldAnnotated(ft.value) ? '✓' : '✗' }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useTemplateStore } from '../stores/template'
import { FIELD_TYPE_LABELS, type InvoiceTemplate, type FieldType } from '../types'

const props = defineProps<{ template: InvoiceTemplate }>()
const emit = defineEmits<{ 'update-field': [fieldName: string, pattern: string] }>()

const templateStore = useTemplateStore()

const fieldTypes = computed(() =>
  (Object.keys(FIELD_TYPE_LABELS) as FieldType[]).map(v => ({ value: v, label: FIELD_TYPE_LABELS[v] }))
)

const pdfPath = ref('')
const ocrText = ref('')
const loadingOcr = ref(false)
const selectedFieldType = ref<FieldType>('Amount')
const generatedRegex = ref<string | null>(null)
const editableRegex = ref('')
const ocrTextRef = ref<HTMLElement | null>(null)

async function selectPdf() {
  const selected = await open({
    filters: [{ name: 'PDF', extensions: ['pdf'] }],
    multiple: false,
  })
  if (selected && typeof selected === 'string') {
    pdfPath.value = selected
    ocrText.value = ''
    generatedRegex.value = null
    loadingOcr.value = true
    try {
      ocrText.value = await templateStore.getOcrText(selected)
    } catch (e) {
      alert(`OCR 失败: ${e}`)
    } finally {
      loadingOcr.value = false
    }
  }
}

function handleTextSelection() {
  const selection = window.getSelection()
  if (!selection || selection.isCollapsed) return

  const selectedText = selection.toString().trim()
  if (!selectedText) return

  // 调用后端生成正则骨架
  templateStore.generateRegex(selectedFieldType.value, selectedText)
    .then(regex => {
      generatedRegex.value = regex
      editableRegex.value = regex
    })
    .catch(e => alert(`生成正则失败: ${e}`))
}

function confirmField() {
  if (!editableRegex.value) return
  // FieldType → 字段名映射
  const fieldNameMap: Record<FieldType, string> = {
    Amount: 'amount',
    Date: 'date',
    InvoiceNumber: 'invoice_number',
    SellerName: 'seller_name',
    ItemName: 'item_name',
  }
  emit('update-field', fieldNameMap[selectedFieldType.value], editableRegex.value)
  generatedRegex.value = null
}

function isFieldAnnotated(ft: FieldType): boolean {
  const fieldNameMap: Record<FieldType, string> = {
    Amount: 'amount',
    Date: 'date',
    InvoiceNumber: 'invoice_number',
    SellerName: 'seller_name',
    ItemName: 'item_name',
  }
  const field = props.template.fields.find(f => f.name === fieldNameMap[ft])
  return !!field && field.strategies.some(s => s.pattern && s.pattern.length > 0)
}
</script>
