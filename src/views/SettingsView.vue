<template>
  <div class="max-w-4xl mx-auto px-5 py-6 pb-10">
    <!-- 标题区 -->
    <div class="flex flex-wrap items-center justify-between gap-3 mb-6 animate-fade-in-up">
      <div>
        <h2 class="font-display text-2xl font-extrabold text-slate-900">报销标准设置</h2>
        <p class="text-sm text-slate-500 mt-1">配置市内交通、伙食补助与住宿标准，保存后立即生效，重启后保留</p>
      </div>
    </div>

    <!-- 加载中 -->
    <div v-if="loading" class="card p-10 text-center text-slate-400 animate-fade-in-up">
      正在加载配置…
    </div>

    <!-- 加载失败 -->
    <div v-else-if="loadError" class="card p-10 text-center animate-fade-in-up">
      <p class="text-sm text-rose-600 mb-3">{{ loadError }}</p>
      <AppButton variant="secondary" size="sm" @click="load">重试</AppButton>
    </div>

    <template v-else-if="config">
      <!-- 卡片 1：基础标准（全局） -->
      <div class="card p-5 mb-6 animate-fade-in-up">
        <div class="flex items-center gap-3 mb-5">
          <span class="w-10 h-10 rounded-xl bg-gradient-to-br from-primary-600 to-accent-500 text-white shadow-glow-sm flex items-center justify-center shrink-0">
            <Settings :size="18" />
          </span>
          <div>
            <h3 class="font-display text-base font-bold text-slate-800">基础标准</h3>
            <p class="text-xs text-slate-400 mt-0.5">全局通用，不属于任何标准集</p>
          </div>
        </div>

        <div class="grid gap-5 sm:grid-cols-2">
          <div>
            <div class="flex items-center gap-2 mb-2">
              <span class="w-7 h-7 rounded-lg bg-primary-50 text-primary-600 flex items-center justify-center shrink-0"><Car :size="14" /></span>
              <label class="text-sm font-medium text-slate-700">市内交通每日上限</label>
            </div>
            <div class="relative">
              <input v-model.number="config.cityTransportDaily" type="number" min="0" class="input !pr-12 tabular-nums" placeholder="80" />
              <span class="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-slate-400 pointer-events-none">元/天</span>
            </div>
            <p class="text-xs text-slate-400 mt-1.5 leading-relaxed">市内打车、地铁等交通费用每日报销上限</p>
          </div>

          <div>
            <div class="flex items-center gap-2 mb-2">
              <span class="w-7 h-7 rounded-lg bg-accent-400/10 text-accent-600 flex items-center justify-center shrink-0"><Utensils :size="14" /></span>
              <label class="text-sm font-medium text-slate-700">伙食补助每日标准</label>
            </div>
            <div class="relative">
              <input v-model.number="config.mealSubsidyDaily" type="number" min="0" class="input !pr-12 tabular-nums" placeholder="100" />
              <span class="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-slate-400 pointer-events-none">元/天</span>
            </div>
            <p class="text-xs text-slate-400 mt-1.5 leading-relaxed">出差期间每日伙食补助金额</p>
          </div>
        </div>
      </div>

      <!-- 卡片 2：标准集管理 -->
      <div class="card p-5 mb-6 animate-fade-in-up" style="animation-delay: 60ms">
        <div class="flex items-center gap-3 mb-5">
          <span class="w-10 h-10 rounded-xl bg-gradient-to-br from-slate-600 to-slate-800 text-white shadow-glow-sm flex items-center justify-center shrink-0">
            <Layers :size="18" />
          </span>
          <div class="min-w-0">
            <h3 class="font-display text-base font-bold text-slate-800">标准集管理</h3>
            <p class="text-xs text-slate-400 mt-0.5">「使用中」的标准集决定当前生效的住宿标准</p>
          </div>
          <AppButton variant="primary" size="sm" class="ml-auto shrink-0" :loading="creating" @click="createFromBuiltin">
            <CopyPlus :size="14" /> 从默认标准创建
          </AppButton>
        </div>

        <div class="space-y-2">
          <!-- 默认标准行（内置，只读） -->
          <div class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl border transition-all"
               :class="isActive('builtin') ? 'border-primary-300 bg-primary-50/70 shadow-card' : 'border-slate-200 bg-white hover:border-primary-200'">
            <button @click="selectSet('builtin')" class="text-sm font-medium text-slate-700 hover:text-primary-600 truncate">默认标准</button>
            <AppBadge tone="info">内置</AppBadge>
            <AppBadge v-if="isActive('builtin')" tone="success">使用中</AppBadge>
            <div class="ml-auto flex items-center gap-1 shrink-0">
              <button @click="activateSet('builtin')"
                      class="w-7 h-7 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 flex items-center justify-center transition-colors shrink-0"
                      title="设为使用中">
                <CheckCircle2 :size="15" />
              </button>
            </div>
          </div>

          <!-- 用户标准集行 -->
          <div v-for="s in config.standardSets" :key="s.id"
               class="flex items-center gap-2.5 px-3 py-2.5 rounded-xl border transition-all"
               :class="isActive(s.id) ? 'border-primary-300 bg-primary-50/70 shadow-card' : 'border-slate-200 bg-white hover:border-primary-200'">
            <template v-if="renamingId === s.id">
              <input v-model="renameInput" class="input-sm flex-1 min-w-0" placeholder="标准集名称"
                     @keyup.enter="confirmRename" @keyup.esc="cancelRename" />
              <button @click="confirmRename"
                      class="w-7 h-7 rounded-lg text-slate-400 hover:text-emerald-600 hover:bg-emerald-50 flex items-center justify-center transition-colors shrink-0"
                      title="确认">
                <Check :size="15" />
              </button>
              <button @click="cancelRename"
                      class="w-7 h-7 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 flex items-center justify-center transition-colors shrink-0"
                      title="取消">
                <X :size="15" />
              </button>
            </template>
            <template v-else>
              <button @click="selectSet(s.id)" class="text-sm font-medium text-slate-700 hover:text-primary-600 truncate" :title="s.name">{{ s.name }}</button>
              <AppBadge v-if="isActive(s.id)" tone="success">使用中</AppBadge>
              <div class="ml-auto flex items-center gap-1 shrink-0">
                <button @click="activateSet(s.id)"
                        class="w-7 h-7 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 flex items-center justify-center transition-colors shrink-0"
                        title="设为使用中">
                  <CheckCircle2 :size="15" />
                </button>
                <button @click="copySet(s)"
                        class="w-7 h-7 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 flex items-center justify-center transition-colors shrink-0"
                        title="复制">
                  <Copy :size="15" />
                </button>
                <button @click="startRename(s)"
                        class="w-7 h-7 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 flex items-center justify-center transition-colors shrink-0"
                        title="重命名">
                  <Pencil :size="15" />
                </button>
                <button @click="requestDeleteSet(s)"
                        class="w-7 h-7 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 flex items-center justify-center transition-colors shrink-0"
                        title="删除">
                  <Trash2 :size="15" />
                </button>
              </div>
            </template>
          </div>

          <p v-if="!config.standardSets.length" class="text-xs text-slate-400 pt-1">
            还没有自定义标准集，点击「从默认标准创建」开始
          </p>
        </div>
      </div>

      <!-- 卡片 3：标准详情 -->
      <div ref="detailCardRef" class="card p-5 animate-fade-in-up" style="animation-delay: 120ms">
        <div class="flex items-center gap-3 mb-4">
          <span class="w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-500 to-teal-500 text-white shadow-glow-sm flex items-center justify-center shrink-0">
            <Building2 :size="18" />
          </span>
          <div class="min-w-0">
            <h3 class="font-display text-base font-bold text-slate-800">标准详情</h3>
            <p class="text-xs text-slate-400 mt-0.5">按省份 → 城市设置每晚上限</p>
          </div>
        </div>

        <!-- 集选择器 -->
        <div class="flex items-center gap-2 mb-5 overflow-x-auto pb-1 -mx-1 px-1">
          <button v-for="opt in setOptions" :key="opt.id" @click="selectSet(opt.id)"
                  class="shrink-0 rounded-full px-3 py-1.5 text-xs font-medium border transition-all"
                  :class="editedSetId === opt.id
                    ? 'bg-primary-600 text-white border-transparent shadow-glow-sm'
                    : 'bg-white text-slate-600 border-slate-200 hover:border-primary-300 hover:text-primary-700'">
            {{ opt.label }}
          </button>
        </div>

        <!-- 内置只读提示 -->
        <div v-if="isBuiltin" class="mb-4 flex items-start gap-2 bg-slate-50 border border-slate-200 rounded-lg px-3 py-2.5">
          <Info :size="14" class="text-slate-400 mt-0.5 shrink-0" />
          <p class="text-xs text-slate-500 leading-relaxed">
            内置标准为软件自带数据，只读；如需调整，可点击「从默认标准创建」生成一套可编辑的新标准集。
          </p>
        </div>

        <!-- 未匹配默认值（用户集专属） -->
        <template v-if="detailSet">
          <div class="flex flex-wrap items-center gap-3 mb-5">
            <label class="text-sm font-medium text-slate-700 shrink-0">未匹配默认值</label>
            <div class="relative w-36 shrink-0">
              <input v-model.number="detailSet.defaultHotelStandard" type="number" min="0" class="input-sm !pr-12 tabular-nums" placeholder="350" />
              <span class="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] text-slate-400 pointer-events-none">元/晚</span>
            </div>
            <p class="text-xs text-slate-400">未匹配到任何省份的住宿按此标准执行</p>
          </div>
        </template>

        <!-- 省份列表 -->
        <div v-if="detailProvinces.length" class="space-y-2.5">
          <div v-for="(p, i) in detailProvinces" :key="p.name + i"
               class="border border-slate-200 rounded-xl overflow-hidden bg-white">
            <!-- 省份头行 -->
            <div class="flex items-center gap-2 px-3 py-2 bg-slate-50/70">
              <button v-if="p.cities.length" @click="toggleProvince(p.name)"
                      class="w-6 h-6 rounded-md text-slate-400 hover:text-primary-600 hover:bg-white flex items-center justify-center shrink-0 transition-colors"
                      :title="collapsedProvinces.has(p.name) ? '展开城市' : '收起城市'">
                <ChevronRight :size="14" class="transition-transform duration-200"
                              :class="collapsedProvinces.has(p.name) ? '' : 'rotate-90'" />
              </button>
              <span v-else class="w-6 shrink-0" />
              <input v-if="!isBuiltin" v-model="p.name" class="input-sm flex-1 min-w-0" placeholder="省份名，如：湖南省" />
              <span v-else class="text-sm font-medium text-slate-700 flex-1 min-w-0 truncate" :title="p.name">{{ p.name }}</span>
              <span v-if="p.cities.length" class="text-xs text-slate-400 shrink-0 tabular-nums">{{ p.cities.length }} 城</span>
              <span class="text-xs text-slate-400 shrink-0">其他城市</span>
              <div class="relative w-28 shrink-0">
                <input v-model.number="p.defaultStandard" :disabled="isBuiltin" type="number" min="0" class="input-sm !pr-12 tabular-nums" />
                <span class="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] text-slate-400 pointer-events-none">元/晚</span>
              </div>
              <button v-if="!isBuiltin" @click="removeProvince(detailProvinces, i)"
                      class="w-7 h-7 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 flex items-center justify-center transition-colors shrink-0"
                      title="删除省份">
                <Trash2 :size="14" />
              </button>
            </div>

            <!-- 城市子行 -->
            <template v-if="!collapsedProvinces.has(p.name)">
              <div v-if="p.cities.length" class="divide-y divide-slate-100">
                <div v-for="(city, j) in p.cities" :key="j" class="flex items-center gap-2 px-3 py-2 pl-10">
                  <MapPin :size="13" class="text-slate-300 shrink-0" />
                  <input v-if="!isBuiltin" v-model="city.name" class="input-sm flex-1 min-w-0" placeholder="城市名，如：长沙" />
                  <span v-else class="text-sm text-slate-600 flex-1 min-w-0 truncate" :title="city.name">{{ city.name }}</span>
                  <div class="relative w-28 shrink-0">
                    <input v-model.number="city.standard" :disabled="isBuiltin" type="number" min="0" class="input-sm !pr-12 tabular-nums" />
                    <span class="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] text-slate-400 pointer-events-none">元/晚</span>
                  </div>
                  <button v-if="!isBuiltin" @click="removeCity(p, j)"
                          class="w-7 h-7 rounded-lg text-slate-400 hover:text-rose-600 hover:bg-rose-50 flex items-center justify-center transition-colors shrink-0"
                          title="删除城市">
                    <X :size="14" />
                  </button>
                </div>
              </div>
              <button v-if="!isBuiltin" @click="addCity(p)"
                      class="w-full text-left pl-10 pr-3 py-2 text-xs text-primary-600 hover:bg-primary-50/60 flex items-center gap-1.5 transition-colors">
                <Plus :size="13" /> 添加城市
              </button>
            </template>
          </div>
        </div>
        <div v-else-if="detailSet"
             class="border border-dashed border-slate-300 rounded-xl py-8 text-center">
          <MapPin :size="24" class="mx-auto text-slate-300 mb-2" />
          <p class="text-sm text-slate-400">该标准集还没有设置省份，点击下方「添加省份」开始</p>
        </div>
        <div v-else class="border border-dashed border-slate-300 rounded-xl py-8 text-center">
          <Database :size="24" class="mx-auto text-slate-300 mb-2" />
          <p class="text-sm text-slate-400">暂无内置标准数据</p>
        </div>

        <!-- 添加省份 -->
        <AppButton v-if="detailSet" variant="secondary" size="sm" class="mt-4" @click="addProvince(detailSet)">
          <Plus :size="14" /> 添加省份
        </AppButton>

        <!-- 优先级说明 -->
        <div v-if="detailSet" class="flex items-start gap-2 mt-5 pt-4 border-t border-slate-100">
          <Info :size="14" class="text-slate-400 mt-0.5 shrink-0" />
          <p class="text-xs text-slate-400 leading-relaxed">
            启用标准集后按该集的标准计算；未设置的省份按上方「未匹配默认值」执行。
          </p>
        </div>
      </div>

      <!-- 保存区：有未保存修改时作为底部悬浮条常驻可见 -->
      <div v-if="dirty" class="sticky bottom-4 z-10 mt-6 -mx-1 px-1 animate-fade-in-up">
        <div class="glass rounded-2xl border-t border-white/60 shadow-card-lg backdrop-blur-xl px-5 py-3 flex flex-wrap items-center justify-between gap-3">
          <span class="text-xs text-amber-600 flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse-soft" />有未保存的修改
          </span>
          <AppButton variant="primary" :loading="saving" @click="handleSave">保存设置</AppButton>
        </div>
      </div>
    </template>

    <ConfirmDialog
      :visible="deleteTarget !== null"
      title="删除标准集"
      :message="deleteMessage"
      confirm-text="删除"
      @confirm="confirmDeleteSet"
      @cancel="deleteTarget = null"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  Settings, Car, Utensils, Layers, CopyPlus, Plus, CheckCircle2, Copy, Pencil,
  Trash2, Check, X, Building2, ChevronRight, MapPin, Info, Database,
} from 'lucide-vue-next'
import AppButton from '../components/ui/AppButton.vue'
import AppBadge from '../components/ui/AppBadge.vue'
import ConfirmDialog from '../components/ui/ConfirmDialog.vue'
import { toast } from '../composables/toast'
import type { ReimbursementConfig, StandardSet, ProvinceStandard } from '../types'

const loading = ref(true)
const loadError = ref('')
const saving = ref(false)
const creating = ref(false)
const config = ref<ReimbursementConfig | null>(null)
// 上次成功保存的快照，用于判断是否有未保存修改
const saved = ref<ReimbursementConfig | null>(null)
// 内置「默认标准」的省份层级派生结构（只读）
const builtin = ref<ProvinceStandard[]>([])

// 当前正在编辑的标准集：'builtin' 表示查看内置默认标准（只读）
const editedSetId = ref<string>('builtin')
// 折叠的省份（按省份名记录，重命名后自动展开，可接受）
const collapsedProvinces = ref<Set<string>>(new Set())
const renamingId = ref('')
const renameInput = ref('')
const deleteTarget = ref<{ id: string; name: string; wasActive: boolean } | null>(null)
const detailCardRef = ref<HTMLElement | null>(null)

const dirty = computed(() =>
  JSON.stringify(config.value) !== JSON.stringify(saved.value)
)

const isBuiltin = computed(() => editedSetId.value === 'builtin')

// 当前编辑的用户标准集；内置模式下为 null
const detailSet = computed<StandardSet | null>(() => {
  if (isBuiltin.value) return null
  return config.value?.standardSets.find(s => s.id === editedSetId.value) ?? null
})

// 详情卡片展示的省份列表（内置只读 / 用户集可编辑，同一份引用）
const detailProvinces = computed<ProvinceStandard[]>(() =>
  isBuiltin.value ? builtin.value : detailSet.value?.provinces ?? []
)

const setOptions = computed(() => [
  { id: 'builtin', label: '默认标准' },
  ...(config.value?.standardSets.map(s => ({ id: s.id, label: s.name })) ?? []),
])

const deleteMessage = computed(() => {
  if (!deleteTarget.value) return ''
  return deleteTarget.value.wasActive
    ? `确定删除标准集「${deleteTarget.value.name}」吗？删除后不可恢复，当前使用中的标准集将自动切换回默认标准。`
    : `确定删除标准集「${deleteTarget.value.name}」吗？删除后不可恢复。`
})

function clone<T>(c: T): T {
  return JSON.parse(JSON.stringify(c))
}

// 汉字按拼音排序（ICU 内置，零依赖）；原地排序，编辑中不改名不动序，避免行跳动丢焦点
const pinyinCompare = new Intl.Collator('zh', { sensitivity: 'base' }).compare
function sortProvinces(list: ProvinceStandard[]) {
  list.sort((a, b) => pinyinCompare(a.name, b.name))
  for (const p of list) p.cities.sort((a, b) => pinyinCompare(a.name, b.name))
}

function isActive(id: string) {
  return config.value?.activeStandardSetId === id
}

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    const [cfg, provinces] = await Promise.all([
      invoke<ReimbursementConfig>('get_reimbursement_config'),
      invoke<ProvinceStandard[]>('get_builtin_hotel_standards'),
    ])
    saved.value = cfg
    config.value = clone(cfg)
    builtin.value = provinces
    sortProvinces(builtin.value)
    // 兼容旧保存的无序数据
    for (const s of config.value.standardSets) sortProvinces(s.provinces)
    editedSetId.value = cfg.activeStandardSetId
    collapsedProvinces.value = new Set()
  } catch (e) {
    loadError.value = '加载配置失败: ' + e
    console.error('加载配置失败:', e)
  } finally {
    loading.value = false
  }
}

function selectSet(id: string) {
  editedSetId.value = id
  collapsedProvinces.value = new Set()
}

function activateSet(id: string) {
  config.value!.activeStandardSetId = id
}

// 从默认标准复制出一套可编辑的新标准集，并设为使用中、滚动到详情
async function createFromBuiltin() {
  creating.value = true
  try {
    const provinces = await invoke<ProvinceStandard[]>('get_builtin_hotel_standards')
    const set: StandardSet = {
      id: crypto.randomUUID(),
      name: '我的标准',
      defaultHotelStandard: 350,
      provinces: clone(provinces),
    }
    config.value?.standardSets.push(set)
    sortProvinces(set.provinces)
    config.value!.activeStandardSetId = set.id
    editedSetId.value = set.id
    collapsedProvinces.value = new Set()
    toast('已创建新标准集，可直接编辑', 'success')
    await nextTick()
    detailCardRef.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  } catch (e) {
    console.error('创建标准集失败:', e)
    toast('创建失败: ' + e, 'error')
  } finally {
    creating.value = false
  }
}

function copySet(set: StandardSet) {
  const copy: StandardSet = {
    ...clone(set),
    id: crypto.randomUUID(),
    name: `${set.name} 副本`,
  }
  config.value?.standardSets.push(copy)
  sortProvinces(copy.provinces)
  editedSetId.value = copy.id
  collapsedProvinces.value = new Set()
}

function startRename(set: StandardSet) {
  renamingId.value = set.id
  renameInput.value = set.name
}

function confirmRename() {
  const set = config.value?.standardSets.find(s => s.id === renamingId.value)
  const name = renameInput.value.trim()
  if (set && !name) {
    toast('名称不能为空', 'error')
    return
  }
  if (set) set.name = name
  renamingId.value = ''
}

function cancelRename() {
  renamingId.value = ''
}

function requestDeleteSet(set: StandardSet) {
  deleteTarget.value = {
    id: set.id,
    name: set.name,
    wasActive: config.value?.activeStandardSetId === set.id,
  }
}

function confirmDeleteSet() {
  const c = config.value
  const t = deleteTarget.value
  if (!c || !t) return
  c.standardSets = c.standardSets.filter(s => s.id !== t.id)
  if (c.activeStandardSetId === t.id) c.activeStandardSetId = 'builtin'
  if (editedSetId.value === t.id) editedSetId.value = 'builtin'
  collapsedProvinces.value = new Set()
  deleteTarget.value = null
}

function addProvince(set: StandardSet) {
  set.provinces.push({ name: '', defaultStandard: set.defaultHotelStandard, cities: [] })
  sortProvinces(set.provinces)
}

function removeProvince(provinces: ProvinceStandard[], i: number) {
  const p = provinces[i]
  provinces.splice(i, 1)
  if (p) collapsedProvinces.value.delete(p.name)
}

function addCity(province: ProvinceStandard) {
  province.cities.push({ name: '', standard: 0 })
  province.cities.sort((a, b) => pinyinCompare(a.name, b.name))
}

function removeCity(province: ProvinceStandard, i: number) {
  province.cities.splice(i, 1)
}

function toggleProvince(name: string) {
  if (collapsedProvinces.value.has(name)) collapsedProvinces.value.delete(name)
  else collapsedProvinces.value.add(name)
}

// 保存前拦截明显非法输入；后端另有 sanitize 兜底
function validate(): string | null {
  const c = config.value
  if (!c) return null
  const base = [
    { label: '市内交通每日上限', value: c.cityTransportDaily },
    { label: '伙食补助每日标准', value: c.mealSubsidyDaily },
  ]
  for (const item of base) {
    if (!Number.isFinite(item.value)) return `${item.label}不是有效数字`
    if (item.value < 0) return `${item.label}不能为负数`
  }
  for (const s of c.standardSets) {
    if (!s.name.trim()) return `存在未命名的标准集`
    if (!Number.isFinite(s.defaultHotelStandard) || s.defaultHotelStandard < 0) {
      return `标准集「${s.name}」的未匹配默认值无效`
    }
    for (const [i, p] of s.provinces.entries()) {
      if (!p.name.trim()) return `标准集「${s.name}」第 ${i + 1} 个省份缺少名称`
      if (!Number.isFinite(p.defaultStandard) || p.defaultStandard < 0) {
        return `标准集「${s.name}」省份「${p.name}」的其他城市标准无效`
      }
      for (const [j, city] of p.cities.entries()) {
        if (!city.name.trim()) return `标准集「${s.name}」省份「${p.name}」第 ${j + 1} 个城市缺少名称`
        if (!Number.isFinite(city.standard) || city.standard < 0) {
          return `标准集「${s.name}」省份「${p.name}」城市「${city.name}」的标准无效`
        }
      }
    }
  }
  return null
}

async function handleSave() {
  const err = validate()
  if (err) {
    toast(err, 'error')
    return
  }
  saving.value = true
  try {
    await invoke('set_reimbursement_config', { config: config.value })
    saved.value = clone(config.value!)
    toast('已保存', 'success')
  } catch (e) {
    console.error('保存配置失败:', e)
    toast('保存失败: ' + e, 'error')
  } finally {
    saving.value = false
  }
}

onMounted(load)
</script>
