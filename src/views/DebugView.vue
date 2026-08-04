<template>
  <div class="max-w-6xl mx-auto">
    <!-- 工具栏 -->
    <div class="bg-white rounded-[10px] border border-gray-200 shadow-sm p-5 mb-4 flex flex-wrap items-center gap-4">
      <button @click="pickPdf"
        class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors text-sm">
        选择 PDF
      </button>
      <span v-if="fileName" class="text-sm text-gray-600 truncate max-w-xs">{{ fileName }}</span>

      <div v-if="pages.length" class="flex items-center gap-2">
        <button @click="prevPage" :disabled="currentPage === 0"
          class="px-2 py-1 border rounded text-sm disabled:opacity-40">上一页</button>
        <span class="text-sm">{{ currentPage + 1 }} / {{ pages.length }}</span>
        <button @click="nextPage" :disabled="currentPage >= pages.length - 1"
          class="px-2 py-1 border rounded text-sm disabled:opacity-40">下一页</button>
      </div>

      <div v-if="pages.length" class="flex items-center gap-3 ml-auto">
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showPdfplumber" class="accent-blue-500">
          <span class="text-blue-600">pdfplumber</span>
        </label>
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showOcr" class="accent-red-500">
          <span class="text-red-600">OCR</span>
        </label>
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showShapes" class="accent-orange-500">
          <span class="text-orange-600">图形</span>
        </label>
        <label class="flex items-center gap-1 text-sm">
          <input type="checkbox" v-model="showCells" class="accent-cyan-500">
          <span class="text-cyan-600">单元格</span>
        </label>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="error" class="bg-red-50 border border-red-200 text-red-700 rounded-lg p-3 mb-4 text-sm">
      {{ error }}
    </div>

    <!-- 加载中 -->
    <div v-if="loading" class="text-center py-12 text-gray-400">正在提取文字…</div>

    <!-- 主区域：PDF 底图 + 文字框叠加 -->
    <div v-if="currentPageData" ref="stageRef"
      @mousemove="updateHoverPos"
      @click="selectedShape = ''"
      class="relative inline-block bg-gray-100 rounded-lg border shadow-sm select-none"
      :style="{ width: displayWidth + 'px' }">
      <img :src="currentPageData.image" :width="displayWidth"
        class="block rounded-lg" draggable="false" />

      <!-- 图形元素层（线条/矩形，在文字框下方，可点击选中） -->
      <div v-if="showShapes" class="absolute inset-0">
        <!-- 矩形（单元格） -->
        <div v-for="(rect, i) in currentPageData.rects" :key="'rect-'+i"
          class="absolute cursor-pointer"
          :class="{ 'ring-2 ring-offset-0': selectedShape === 'rect-'+i }"
          :style="rectStyle(rect, selectedShape === 'rect-'+i)"
          @click.stop="selectShape('rect-'+i)"></div>
        <!-- 线条（表格线） -->
        <div v-for="(line, i) in currentPageData.lines" :key="'line-'+i"
          class="absolute cursor-pointer"
          :style="lineStyle(line, selectedShape === 'line-'+i)"
          @click.stop="selectShape('line-'+i)"></div>
      </div>
      <!-- 单元格层（find_tables 识别结果，虚线边框 + 文本标签） -->
      <div v-if="showCells" class="absolute inset-0">
        <div v-for="(cell, i) in currentPageData.cells" :key="'cell-'+i"
          class="absolute cursor-pointer"
          :class="{ 'ring-2 ring-offset-0': selectedShape === 'cell-'+i }"
          :style="cellStyle(cell, selectedShape === 'cell-'+i)"
          @click.stop="selectShape('cell-'+i)"
          @mouseenter="hoveredCell = i"
          @mouseleave="hoveredCell = -1">
          <span v-if="cell.text && (hoveredCell === i || selectedShape === 'cell-'+i)"
            class="absolute left-0 top-0 px-1 py-0.5 leading-snug whitespace-pre-wrap bg-cyan-100/90 text-cyan-900 rounded-sm pointer-events-none z-20"
            style="font-size: 12px;">
            {{ cell.text }}
          </span>
        </div>
      </div>
      <!-- 文字框层 -->
      <div class="absolute inset-0">
        <div v-for="(item, idx) in visibleBoxes" :key="item.key"
          class="absolute border-2 cursor-move"
          :style="boxStyle(item)"
          @mousedown="startDrag($event, idx)"
          @mouseenter="hoveredIdx = idx"
          @mouseleave="hoveredIdx = -1">
          <span class="absolute left-0 top-0 px-0.5 text-xs leading-tight whitespace-nowrap overflow-hidden"
            :style="{ fontSize: Math.max(8, item.h * scale) + 'px', lineHeight: (item.h * scale) + 'px', color: item.color, maxWidth: (item.w * scale) + 'px' }">
            {{ item.text }}
          </span>
        </div>
      </div>

      <!-- 悬停坐标提示（跟随鼠标） -->
      <div v-if="hoveredIdx >= 0 && visibleBoxes[hoveredIdx]"
        class="absolute bg-black/70 text-white text-xs px-2 py-1 rounded pointer-events-none z-10"
        :style="{ left: (hoverPos.x + 12) + 'px', top: (hoverPos.y + 12) + 'px' }">
        {{ visibleBoxes[hoveredIdx].engine }}: "{{ visibleBoxes[hoveredIdx].text }}" x={{ visibleBoxes[hoveredIdx].origX.toFixed(1) }} y={{ visibleBoxes[hoveredIdx].origY.toFixed(1) }} w={{ visibleBoxes[hoveredIdx].origW.toFixed(1) }} h={{ visibleBoxes[hoveredIdx].origH.toFixed(1) }}
      </div>
    </div>

    <p v-if="currentPageData && !visibleBoxes.length" class="text-sm text-gray-400 mt-4">
      当前页无可显示文字框（勾选上方引擎或检查提取结果）。
    </p>

    <!-- 日志面板 -->
    <div v-if="logs.pdfplumber.length || logs.ocr.length" class="mt-4">
      <button @click="showLogs = !showLogs"
        class="text-sm text-gray-500 hover:text-gray-700 flex items-center gap-1">
        <span>{{ showLogs ? '▼' : '▶' }}</span>
        <span>诊断日志</span>
      </button>
      <div v-if="showLogs" class="bg-gray-900 rounded-lg border mt-2">
        <!-- 引擎 tab -->
        <div class="flex border-b border-gray-700">
          <button v-for="eng in (['pdfplumber', 'ocr'] as const)" :key="eng"
            @click="activeLogTab = eng"
            class="px-3 py-1.5 text-xs font-mono transition-colors"
            :class="activeLogTab === eng ? 'bg-gray-800 text-white' : 'text-gray-400 hover:text-gray-200'">
            <span :style="{ color: ENGINE_COLOR_HEX[eng] }">●</span>
            {{ eng }}
            <span class="text-gray-500">({{ logs[eng].length }})</span>
          </button>
        </div>
        <!-- 日志内容 -->
        <div class="p-3 font-mono text-xs text-gray-300 max-h-64 overflow-y-auto">
          <div v-for="(line, i) in logs[activeLogTab]" :key="i" class="whitespace-pre-wrap break-all">
            {{ line }}
          </div>
          <div v-if="!logs[activeLogTab].length" class="text-gray-500">无日志</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

interface DebugTextItem {
  text: string
  x: number
  y: number
  w: number
  h: number
  confidence: number
}
interface DebugLine {
  x0: number
  y0: number
  x1: number
  y1: number
  line_width: number
}
interface DebugRect {
  x: number
  y: number
  w: number
  h: number
  line_width: number
  fill: boolean
}
interface DebugCell {
  x: number
  y: number
  w: number
  h: number
  text: string
}
interface DebugPage {
  image: string
  width: number
  height: number
  pdfplumber: DebugTextItem[]
  ocr: DebugTextItem[]
  lines: DebugLine[]
  rects: DebugRect[]
  cells: DebugCell[]
}
interface DebugLogs {
  pdfplumber: string[]
  ocr: string[]
}
interface DebugTextResult {
  pages: DebugPage[]
  logs: DebugLogs
}

const ENGINE_COLOR_HEX = {
  pdfplumber: '#2563eb',
  ocr: '#dc2626',
}

const fileName = ref('')
const loading = ref(false)
const error = ref('')
const pages = ref<DebugPage[]>([])
const currentPage = ref(0)
const displayWidth = ref(900)

const showPdfplumber = ref(true)
const showOcr = ref(true)
const showShapes = ref(true)
const showCells = ref(true)
const selectedShape = ref('')  // 'rect-3' / 'line-5' / 'cell-2' / ''

const hoveredIdx = ref(-1)
const hoveredCell = ref(-1)
const stageRef = ref<HTMLElement | null>(null)
const hoverPos = ref({ x: 0, y: 0 })

const logs = ref<DebugLogs>({ pdfplumber: [], ocr: [] })
const activeLogTab = ref<'pdfplumber' | 'ocr'>('pdfplumber')
const showLogs = ref(false)

// 拖动状态：每个引擎独立的偏移量（按 engine+index 标识）
// ponytail: 临时拖动不保存，切换页面/引擎时重置
const dragOffsets = ref<Record<string, { dx: number; dy: number }>>({})
const dragState = ref<{ key: string; startMouseX: number; startMouseY: number; startDx: number; startDy: number } | null>(null)

const currentPageData = computed(() => pages.value[currentPage.value] ?? null)

const scale = computed(() => {
  const p = currentPageData.value
  if (!p || p.width === 0) return 1
  return displayWidth.value / p.width
})

interface VisibleBox extends DebugTextItem {
  engine: string
  color: string
  key: string
  origX: number
  origY: number
  origW: number
  origH: number
  offsetX: number
  offsetY: number
}

const visibleBoxes = computed<VisibleBox[]>(() => {
  const p = currentPageData.value
  if (!p) return []
  const out: VisibleBox[] = []
  const push = (items: DebugTextItem[], engine: string) => {
    for (let i = 0; i < items.length; i++) {
      const it = items[i]
      const key = `${engine}-${currentPage.value}-${i}`
      const off = dragOffsets.value[key] ?? { dx: 0, dy: 0 }
      out.push({
        ...it,
        engine,
        key,
        color: ENGINE_COLOR_HEX[engine as keyof typeof ENGINE_COLOR_HEX],
        origX: it.x, origY: it.y, origW: it.w, origH: it.h,
        offsetX: off.dx, offsetY: off.dy,
      })
    }
  }
  if (showPdfplumber.value) push(p.pdfplumber, 'pdfplumber')
  if (showOcr.value) push(p.ocr, 'ocr')
  return out
})

function boxStyle(item: VisibleBox) {
  const s = scale.value
  return {
    left: (item.x * s + item.offsetX) + 'px',
    top: (item.y * s + item.offsetY) + 'px',
    width: (item.w * s) + 'px',
    height: (item.h * s) + 'px',
    borderColor: item.color,
    backgroundColor: item.color + '15',
  }
}

function rectStyle(rect: DebugRect, selected: boolean) {
  const s = scale.value
  const color = selected ? '#a855f7' : '#f97316'
  return {
    left: (rect.x * s) + 'px',
    top: (rect.y * s) + 'px',
    width: (rect.w * s) + 'px',
    height: (rect.h * s) + 'px',
    border: Math.max(1, rect.line_width * s) + 'px solid ' + color,
    backgroundColor: rect.fill ? (selected ? 'rgba(168,85,247,0.2)' : 'rgba(249,115,22,0.1)') : 'transparent',
    zIndex: 0,
  }
}

function lineStyle(line: DebugLine, selected: boolean) {
  const s = scale.value
  const isHorizontal = Math.abs(line.y1 - line.y0) < Math.abs(line.x1 - line.x0)
  // ponytail: 最小命中 4px，细线也能点中；选中加粗到 6px
  const lw = (selected ? Math.max(6, line.line_width * s * 2) : Math.max(4, line.line_width * s))
  const color = selected ? '#a855f7' : '#f97316'
  if (isHorizontal) {
    return {
      left: (line.x0 * s) + 'px',
      top: (line.y0 * s) + 'px',
      width: ((line.x1 - line.x0) * s) + 'px',
      height: lw + 'px',
      backgroundColor: color,
      zIndex: 10,
    }
  } else {
    return {
      left: (line.x0 * s) + 'px',
      top: (line.y0 * s) + 'px',
      width: lw + 'px',
      height: ((line.y1 - line.y0) * s) + 'px',
      backgroundColor: color,
      zIndex: 10,
    }
  }
}

function selectShape(key: string) {
  selectedShape.value = selectedShape.value === key ? '' : key
}

function cellStyle(cell: DebugCell, selected: boolean) {
  const s = scale.value
  const color = selected ? '#a855f7' : '#06b6d4'
  return {
    left: (cell.x * s) + 'px',
    top: (cell.y * s) + 'px',
    width: (cell.w * s) + 'px',
    height: (cell.h * s) + 'px',
    border: '2px dashed ' + color,
    backgroundColor: selected ? 'rgba(168,85,247,0.1)' : 'rgba(6,182,212,0.05)',
    zIndex: 5,
  }
}

function updateHoverPos(e: MouseEvent) {
  const stage = stageRef.value
  if (!stage) return
  const rect = stage.getBoundingClientRect()
  hoverPos.value = { x: e.clientX - rect.left, y: e.clientY - rect.top }
}

function startDrag(e: MouseEvent, idx: number) {
  const box = visibleBoxes.value[idx]
  if (!box) return
  const off = dragOffsets.value[box.key] ?? { dx: 0, dy: 0 }
  dragState.value = {
    key: box.key,
    startMouseX: e.clientX,
    startMouseY: e.clientY,
    startDx: off.dx,
    startDy: off.dy,
  }
  e.preventDefault()
}

function onMouseMove(e: MouseEvent) {
  const ds = dragState.value
  if (!ds) return
  dragOffsets.value = {
    ...dragOffsets.value,
    [ds.key]: {
      dx: ds.startDx + (e.clientX - ds.startMouseX),
      dy: ds.startDy + (e.clientY - ds.startMouseY),
    },
  }
}

function onMouseUp() {
  dragState.value = null
}

window.addEventListener('mousemove', onMouseMove)
window.addEventListener('mouseup', onMouseUp)
onUnmounted(() => {
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
})

function prevPage() {
  if (currentPage.value > 0) {
    currentPage.value--
    dragOffsets.value = {}
    selectedShape.value = ''
    hoveredCell.value = -1
  }
}
function nextPage() {
  if (currentPage.value < pages.value.length - 1) {
    currentPage.value++
    dragOffsets.value = {}
    selectedShape.value = ''
    hoveredCell.value = -1
  }
}

async function pickPdf() {
  error.value = ''
  try {
    const filePath = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (!filePath) return
    fileName.value = filePath.split(/[\\/]/).pop() ?? filePath
    loading.value = true
    pages.value = []
    currentPage.value = 0
    dragOffsets.value = {}
    selectedShape.value = ''
    logs.value = { pdfplumber: [], ocr: [] }
    const result = await invoke<DebugTextResult>('debug_extract_texts', {
      filePath,
      dpi: 200,
    })
    pages.value = result.pages
    logs.value = result.logs ?? { pdfplumber: [], ocr: [] }
    if (!result.pages.length) error.value = '未提取到任何页面'
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}
</script>
