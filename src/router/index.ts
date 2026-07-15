import { createRouter, createWebHashHistory } from 'vue-router'

const routes = [
  { path: '/', name: 'home', component: () => import('../views/HomeView.vue') },
  { path: '/import', name: 'import', component: () => import('../views/ImportView.vue') },
  { path: '/match', name: 'match', component: () => import('../views/MatchView.vue') },
  { path: '/export', name: 'export', component: () => import('../views/ExportView.vue') },
  { path: '/debug', name: 'debug', component: () => import('../views/DebugView.vue') },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})
