<template>
  <div class="max-w-4xl mx-auto">
    <div class="flex justify-between items-center mb-6">
      <h2 class="text-2xl font-bold">模板管理</h2>
      <button @click="handleCreate"
              class="px-4 py-2 rounded bg-blue-600 text-white hover:bg-blue-700 transition-colors text-sm font-medium">
        + 新建模板
      </button>
    </div>

    <div v-if="templateStore.loading" class="text-center py-8 text-gray-400">加载中...</div>

    <template v-else>
      <!-- 内置模板 -->
      <div v-if="builtinTemplates.length" class="mb-8">
        <h3 class="text-sm font-medium text-gray-500 mb-3">内置模板（只读）</h3>
        <div class="grid gap-3">
          <TemplateCard v-for="t in builtinTemplates" :key="t.template_id" :template="t"
            @toggle="handleToggle" @test="handleTest" @copy="handleCopy" @edit="handleEdit" @delete="handleDelete" />
        </div>
      </div>

      <!-- 用户模板 -->
      <div>
        <h3 class="text-sm font-medium text-gray-500 mb-3">我的模板</h3>
        <div v-if="userTemplates.length" class="grid gap-3">
          <TemplateCard v-for="t in userTemplates" :key="t.template_id" :template="t"
            @toggle="handleToggle" @test="handleTest" @copy="handleCopy" @edit="handleEdit" @delete="handleDelete" />
        </div>
        <div v-else class="text-center py-8 text-gray-400 border border-dashed rounded-lg">
          暂无自定义模板，点击右上角"新建模板"创建
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useTemplateStore } from '../stores/template'
import TemplateCard from '../components/TemplateCard.vue'

const router = useRouter()
const templateStore = useTemplateStore()

const builtinTemplates = computed(() => templateStore.templates.filter(t => t.source === 'Builtin'))
const userTemplates = computed(() => templateStore.templates.filter(t => t.source === 'User'))

onMounted(() => templateStore.loadTemplates())

async function handleToggle(id: string, enabled: boolean) {
  try {
    await templateStore.toggleTemplate(id, enabled)
  } catch (e) {
    alert(`切换失败: ${e}`)
  }
}

function handleTest(id: string) {
  router.push({ name: 'template-edit', params: { id }, query: { test: '1' } })
}

async function handleCopy(id: string) {
  // 加载内置模板，改 id 后跳转编辑器
  try {
    await templateStore.loadTemplate(id)
    if (templateStore.currentTemplate) {
      const copy = { ...templateStore.currentTemplate, template_id: 'user_' + Date.now(), name: templateStore.currentTemplate.name + ' (副本)' }
      await templateStore.saveTemplate(copy)
    }
  } catch (e) {
    alert(`复制失败: ${e}`)
  }
}

function handleEdit(id: string) {
  router.push({ name: 'template-edit', params: { id } })
}

async function handleDelete(id: string) {
  if (!confirm('确定删除此模板？')) return
  try {
    await templateStore.deleteTemplate(id)
  } catch (e) {
    alert(`删除失败: ${e}`)
  }
}

function handleCreate() {
  router.push({ name: 'template-edit' })
}
</script>
