import { createRouter, createWebHashHistory } from 'vue-router'

const routes = [
  { path: '/', name: 'home', component: () => import('../views/HomeView.vue') },
  { path: '/import', name: 'import', component: () => import('../views/ImportView.vue') },
  { path: '/match', name: 'match', component: () => import('../views/MatchView.vue') },
  { path: '/export', name: 'export', component: () => import('../views/ExportView.vue') },
  { path: '/templates', name: 'templates', component: () => import('../views/TemplateListView.vue') },
  { path: '/templates/edit/:id?', name: 'template-edit', component: () => import('../views/TemplateEditorView.vue') },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
